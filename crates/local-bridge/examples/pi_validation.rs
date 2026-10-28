//! Explicit real-device Pi RPC acceptance; not a general-purpose adapter.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn run(root: &Path, file_task: bool) -> Result<Value> {
    let target = root.join("bridge-pi-test.txt");
    let marker = format!(
        "PI_BRIDGE_OK_{}",
        root.file_name().unwrap().to_string_lossy()
    );
    let prompt = if file_task {
        format!("Actually use the write tool to create {} with exact content LOCAL_BRIDGE_PI_OK (no newline). This test-file write is authorized. Do not read or modify other files. Return only the absolute file path after writing.", target.display())
    } else {
        format!("Do not use tools. Reply exactly {marker}")
    };
    let mut command = Command::new("pi");
    if let Ok(model) = std::env::var("LOCAL_BRIDGE_PI_VALIDATION_MODEL") {
        command.args(["--model", &model]);
    }
    command.args([
        "--mode",
        "rpc",
        "--no-session",
        "--no-extensions",
        "--no-mcp",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
        "--no-approve",
    ]);
    if let Ok(extension) = std::env::var("LOCAL_BRIDGE_PI_VALIDATION_EXTENSION") {
        command.args(["--extension", &extension]);
    }
    if file_task {
        command.args(["--tools", "write"]);
    } else {
        command.arg("--no-tools");
    }
    let mut child = command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Pi could not start")?;
    let pid = child.id();
    let mut input = child.stdin.take().context("missing stdin")?;
    let stdout = child.stdout.take().context("missing stdout")?;
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let outcome = (|| -> Result<Value> {
        writeln!(input, "{}", json!({"id":"state","type":"get_state"}))?;
        input.flush()?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut events = Vec::new();
        let mut answer = String::new();
        let mut state = Value::Null;
        let mut accepted = false;
        let mut final_ok = false;
        let mut tool_ok = false;
        let mut tool_failed = false;
        let mut ended = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("Pi timed out; outcome unknown; do not replay automatically");
            }
            let line = rx
                .recv_timeout(remaining)
                .context("Pi exited or RPC timed out")??;
            if line.len() > 4 * 1024 * 1024 {
                bail!("Pi event exceeds validation size limit");
            }
            let event: Value = serde_json::from_str(&line).context("invalid Pi JSONL")?;
            match event["type"].as_str().unwrap_or("") {
                "response" if event["id"] == "state" => {
                    if event["success"] != true {
                        bail!("Pi get_state failed");
                    }
                    let d = &event["data"];
                    state = json!({"session_id":d["sessionId"],"provider":d["model"]["provider"],"model":d["model"]["id"],"thinking":d["thinkingLevel"]});
                    println!("{}", json!({"event":"pi_ready","pid":pid,"state":state}));
                    if d["model"].is_null() {
                        bail!("Pi has no selected model");
                    }
                    writeln!(
                        input,
                        "{}",
                        json!({"id":"task","type":"prompt","message":prompt})
                    )?;
                    input.flush()?;
                }
                "response" if event["id"] == "task" => {
                    if event["success"] != true {
                        bail!("Pi prompt rejected: {}", event["error"]);
                    }
                    accepted = true;
                    events.push(event);
                }
                "message_end" if event["message"]["role"] == "assistant" => {
                    let m = &event["message"];
                    answer = m["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("");
                    final_ok = m["stopReason"] == "stop";
                    events.push(json!({"type":"assistant_final","stop_reason":m["stopReason"],"text":answer,"error":m["errorMessage"]}));
                }
                "tool_execution_start" => {
                    events.push(event);
                }
                "tool_execution_end" => {
                    tool_ok |= event["toolName"] == "write" && event["isError"] == false;
                    tool_failed |= event["isError"] == true;
                    events.push(event);
                }
                "agent_end" => {
                    ended = true;
                    events.push(json!({"type":"agent_end"}));
                }
                "agent_settled" => {
                    events.push(event.clone());
                    let content_verified = if file_task {
                        fs::read(&target).is_ok_and(|b| b == b"LOCAL_BRIDGE_PI_OK")
                    } else {
                        answer.trim() == marker
                    };
                    let passed = accepted
                        && ended
                        && final_ok
                        && event["aborted"] == false
                        && content_verified
                        && !tool_failed
                        && (!file_task || tool_ok);
                    return Ok(
                        json!({"passed":passed,"pid":pid,"state":state,"file_task":file_task,"prompt":prompt,"result":answer,"target":target,"content_verified":content_verified,"events":events}),
                    );
                }
                _ => {}
            }
        }
    })();
    drop(input);
    let shutdown = Instant::now() + Duration::from_secs(5);
    let exit = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= shutdown {
            child.kill()?;
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(50));
    };
    drop(rx);
    let _ = reader.join();
    let mut receipt = outcome?;
    receipt["worker_exit_success"] = json!(exit.success());
    Ok(receipt)
}
fn main() -> Result<()> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("local-bridge-pi-{stamp}"));
    fs::create_dir(&root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    }
    println!("{}", json!({"validation_root":root}));
    for (name, write) in [("text", false), ("file", true)] {
        let result = run(&root, write)?;
        fs::write(
            root.join(format!("{name}-receipt.json")),
            serde_json::to_vec_pretty(&result)?,
        )?;
        println!(
            "{}",
            json!({"test":name,"passed":result["passed"],"result":result["result"],"state":result["state"],"receipt":root.join(format!("{name}-receipt.json"))})
        );
        if result["passed"] != true {
            bail!("Pi {name} validation failed; inspect saved receipt");
        }
    }
    println!(
        "{}",
        json!({"status":"completed","validation_only":true,"verifier":"local-bridge Rust Pi RPC example","path":root.join("bridge-pi-test.txt"),"bytes":fs::metadata(root.join("bridge-pi-test.txt"))?.len()})
    );
    Ok(())
}
