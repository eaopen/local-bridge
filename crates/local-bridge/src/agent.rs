//! Explicit user-invoked Codex CLI integration. It is **not** a remote
//! callable MCP tool: only a local CLI command or a deliberate click in the
//! trusted Chrome extension popup may start this fixed read-only task.
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

// macOS Chrome launched from Finder has a minimal PATH; a user's existing
// Homebrew Codex CLI (and its /usr/bin/env node interpreter) can otherwise be
// invisible. Only add conventional local tool directories to *this child*.
// Never run a shell or change global/user PATH settings.
fn codex_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command
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
        .stderr(Stdio::null());
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        let raw = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&raw).collect();
        for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
            let path = PathBuf::from(dir);
            if path.is_dir() && !dirs.contains(&path) {
                dirs.push(path);
            }
        }
        if let Ok(paths) = std::env::join_paths(dirs) {
            command.env("PATH", paths);
        }
    }
    command
}

fn spawn_codex() -> Result<std::process::Child> {
    // Prefer the user's PATH, and on macOS fall back to existing Homebrew
    // launchers for Chrome's GUI environment. No installation is performed.
    #[cfg(target_os = "macos")]
    let candidates = ["codex", "/opt/homebrew/bin/codex", "/usr/local/bin/codex"];
    #[cfg(not(target_os = "macos"))]
    let candidates = ["codex"];

    for candidate in candidates {
        match codex_command(candidate).spawn() {
            Ok(child) => return Ok(child),
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).context("Codex CLI could not start"),
        }
    }
    bail!("Codex CLI not available; install and log in to Codex first")
}

/// Run only the user's explicitly requested Codex task in its documented
/// non-interactive, ephemeral, read-only mode. No arbitrary executable or
/// arguments can arrive from a Profile, browser page, MCP, or model text.
pub fn run_codex(prompt: &str) -> Result<Value> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT {
        bail!("agent prompt must contain 1..65536 bytes of text");
    }

    let mut child = spawn_codex()?;

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

// CodeBuddy Code is a separate, officially supported CLI. Logging in once
// with the user's existing Buddy account can use the shared subscription
// credits, but does NOT inherit the running WorkBuddy desktop's GUI, skills,
// session, or login credentials. This entrypoint is deliberately text-only:
// no --dangerously-skip-permissions, no built-in tools, and no MCP tools.
fn parse_codebuddy_result(output: &str) -> Result<String> {
    let event: Value =
        serde_json::from_str(output.trim()).context("invalid CodeBuddy headless JSON response")?;
    if event["type"] != "result" || event["subtype"] != "success" || event["is_error"] != false {
        bail!("CodeBuddy did not return an explicit successful final result");
    }
    let answer = event["result"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .context("CodeBuddy did not return a non-empty completed response")?;
    Ok(answer.to_owned())
}

fn codebuddy_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .args([
            "-p",
            "--output-format",
            "json",
            "--tools",
            "",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--no-session-persistence",
        ])
        // A one-shot result must never hide a running background agent task.
        .env("CODEBUDDY_CODE_DISABLE_BACKGROUND_TASKS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        let raw = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&raw).collect();
        for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
            let path = PathBuf::from(dir);
            if path.is_dir() && !dirs.contains(&path) {
                dirs.push(path);
            }
        }
        if let Ok(paths) = std::env::join_paths(dirs) {
            command.env("PATH", paths);
        }
    }
    command
}

fn spawn_codebuddy() -> Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    let candidates = [
        "codebuddy",
        "cbc",
        "/opt/homebrew/bin/codebuddy",
        "/opt/homebrew/bin/cbc",
        "/usr/local/bin/codebuddy",
        "/usr/local/bin/cbc",
    ];
    #[cfg(target_os = "windows")]
    let candidates = ["codebuddy", "cbc", "codebuddy.cmd", "cbc.cmd"];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let candidates = ["codebuddy", "cbc"];

    for candidate in candidates {
        match codebuddy_command(candidate).spawn() {
            Ok(child) => return Ok(child),
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).context("CodeBuddy CLI could not start"),
        }
    }
    bail!("CodeBuddy CLI not found; install CodeBuddy Code and log in separately with your Buddy account")
}

/// Explicit local read-only CodeBuddy task. A successful process start, a
/// desktop subscription check, or an IPC hello never count as a completed run.
pub fn run_codebuddy(prompt: &str) -> Result<Value> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT {
        bail!("agent prompt must contain 1..65536 bytes of text");
    }
    let mut child = spawn_codebuddy()?;
    let stdout = child
        .stdout
        .take()
        .context("CodeBuddy stdout unavailable")?;
    let reader = thread::spawn(move || -> io::Result<Option<Vec<u8>>> {
        let mut stdout = stdout;
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        let mut too_large = false;
        loop {
            let n = stdout.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            if !too_large {
                if bytes.len() + n > MAX_EVENTS {
                    too_large = true;
                    bytes.clear();
                } else {
                    bytes.extend_from_slice(&buffer[..n]);
                }
            }
        }
        Ok((!too_large).then_some(bytes))
    });

    let write_result = child
        .stdin
        .take()
        .context("CodeBuddy stdin unavailable")?
        .write_all(prompt.as_bytes());
    if write_result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        bail!("CodeBuddy could not accept the prompt");
    }
    let deadline = Instant::now() + RUN_TIMEOUT;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .context("CodeBuddy process status unavailable")?
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            bail!("CodeBuddy run timed out; completion is unknown; do not retry automatically");
        }
        thread::sleep(Duration::from_millis(100));
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("CodeBuddy output reader failed"))??
        .context("CodeBuddy response exceeded size limit")?;
    if !status.success() {
        bail!("CodeBuddy did not complete; check its own CLI sign-in and subscription");
    }
    let response = std::str::from_utf8(&bytes).context("CodeBuddy response not UTF-8")?;
    let answer = parse_codebuddy_result(response)?;
    Ok(json!({
        "source": "local-cli",
        "owner_module": "ai.codebuddy",
        "status": "completed",
        "result": answer,
        "persisted_by_bridge": false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codebuddy_requires_a_real_final_answer() {
        let ok = r#"{"type":"result","subtype":"success","is_error":false,"result":"PONG"}"#;
        assert_eq!(parse_codebuddy_result(ok).unwrap(), "PONG");
        for bad in [
            r#"{"type":"result","is_error":true,"result":"login required"}"#,
            r#"{"type":"result","subtype":"error","result":"failed"}"#,
            r#"{"type":"system","result":"not final"}"#,
            r#"{"type":"result","is_error":false}"#,
            r#"{"type":"result","result":"  "}"#,
            r#"{"result":"not a completion event"}"#,
            r#"{"type":"result","subtype":"error_max_turns","is_error":false,"result":"partial"}"#,
            "invalid",
        ] {
            assert!(parse_codebuddy_result(bad).is_err(), "{bad}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_gui_codex_child_path_can_find_homebrew_node() {
        let command = codex_command("/opt/homebrew/bin/codex");
        let child_path = command
            .get_envs()
            .find_map(|(name, value)| (name == "PATH").then_some(value).flatten())
            .expect("macOS child must get an augmented PATH");
        let dirs: Vec<_> = std::env::split_paths(child_path).collect();
        if std::path::Path::new("/opt/homebrew/bin").is_dir() {
            assert!(dirs.contains(&std::path::PathBuf::from("/opt/homebrew/bin")));
        }
    }

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
