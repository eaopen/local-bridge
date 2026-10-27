//! Explicit user-invoked Codex CLI integration. This is intentionally **not**
//! a remote-callable MCP tool: a webpage/message cannot start local processes.
//! Codex is the user's existing installed/authenticated CLI; no credentials,
//! file versions, or agent sessions are owned by Local Bridge.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_PROMPT: usize = 64 * 1024;
const MAX_EVENTS: usize = 4 * 1024 * 1024;
const RUN_TIMEOUT: Duration = Duration::from_secs(180);

/// A piped-in message or selected chat export can be used without a new
/// connector protocol. The user explicitly chooses to pass these bytes.
pub fn prompt_from_stdin() -> Result<String> {
    let mut input = Vec::new();
    io::stdin()
        .take((MAX_PROMPT + 1) as u64)
        .read_to_end(&mut input)?;
    if input.len() > MAX_PROMPT {
        bail!("agent prompt too long");
    }
    String::from_utf8(input).context("agent prompt must be UTF-8 text")
}

fn parse_codex_events(output: &str) -> Result<String> {
    let mut completed = false;
    let mut failed = false;
    let mut answer = String::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let event: Value = serde_json::from_str(line).context("invalid Codex JSONL")?;
        match event["type"].as_str() {
            Some("item.completed") if event["item"]["type"] == "agent_message" => {
                if let Some(text) = event["item"]["text"].as_str() {
                    answer = text.to_owned();
                }
            }
            Some("turn.completed") => completed = true,
            Some("turn.failed") => failed = true,
            _ => {}
        }
    }

    if failed || !completed || answer.trim().is_empty() {
        bail!("Codex did not confirm a completed turn with a response");
    }
    Ok(answer)
}

/// Run only the user's explicitly requested Codex task in its documented
/// non-interactive, ephemeral, read-only mode. No arbitrary executable or
/// arguments can arrive from a Profile, browser page, MCP, or model text.
pub fn run_codex(prompt: &str) -> Result<Value> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT {
        bail!("agent prompt must contain 1..65536 bytes of text");
    }

    let mut child = Command::new("codex")
        .args([
            "exec",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--skip-git-repo-check",
            "--json",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Codex CLI not available; install and log in to Codex first")?;

    let stdout = child.stdout.take().context("Codex stdout unavailable")?;
    let reader = thread::spawn(move || -> io::Result<Option<Vec<u8>>> {
        let mut stdout = stdout;
        let mut result = Vec::new();
        let mut buf = [0_u8; 8192];
        let mut too_large = false;
        loop {
            let size = stdout.read(&mut buf)?;
            if size == 0 {
                break;
            }
            if !too_large {
                if result.len() + size > MAX_EVENTS {
                    too_large = true;
                    result.clear();
                } else {
                    result.extend_from_slice(&buf[..size]);
                }
            }
        }
        Ok((!too_large).then_some(result))
    });

    let written = child
        .stdin
        .take()
        .context("Codex stdin unavailable")?
        .write_all(prompt.as_bytes());
    if written.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        bail!("Codex could not accept the prompt");
    }

    let deadline = Instant::now() + RUN_TIMEOUT;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .context("Codex process status unavailable")?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            bail!("Codex run timed out; no completion was confirmed");
        }
        thread::sleep(Duration::from_millis(100));
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Codex output reader failed"))??
        .context("Codex JSONL output exceeded size limit")?;
    if !status.success() {
        bail!("Codex did not complete; check its local login and CLI configuration");
    }
    let transcript = std::str::from_utf8(&bytes).context("Codex JSONL not UTF-8")?;
    let answer = parse_codex_events(transcript)?;

    Ok(json!({
        "source": "local-cli",
        "owner_module": "ai.codex",
        "status": "completed",
        "result": answer,
        "persisted_by_bridge": false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_completed_turn_counts() {
        let stream = [
            r#"{"type":"thread.started","thread_id":"sample"}"#,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"done"}}"#,
            r#"{"type":"turn.completed"}"#,
        ]
        .join("\n");
        assert_eq!(parse_codex_events(&stream).unwrap(), "done");
        assert!(parse_codex_events(
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"partial"}}"#
        )
        .is_err());
        assert!(parse_codex_events(&format!("{stream}\n{{\"type\":\"turn.failed\"}}")).is_err());
    }

    #[test]
    fn rejects_bad_json_or_non_answer_items() {
        assert!(parse_codex_events("not json").is_err());
        assert!(parse_codex_events(
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"reasoning\",\"text\":\"secret\"}}\n{\"type\":\"turn.completed\"}"
        )
        .is_err());
    }
}
