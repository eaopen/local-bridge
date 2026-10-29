//! Explicit user-invoked Codex CLI integration. It is **not** a remote
//! callable MCP tool: only a local CLI command or a deliberate click in the
//! trusted Chrome extension popup may start this fixed read-only task.
//! Codex is the user's existing installed/authenticated CLI; no credentials,
//! file versions, or agent sessions are owned by Local Bridge.

use crate::pi_enterprise;
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
fn codex_command(program: &str, model: &str, think_mode: &str) -> Result<Command> {
    if model != "auto" && !pi_id(model, true) {
        bail!("invalid Codex model");
    }
    if think_mode != "auto" && !["low", "medium", "high", "xhigh"].contains(&think_mode) {
        bail!("unsupported Codex think mode");
    }
    let mut command = Command::new(program);
    command.args([
        "exec",
        "--ephemeral",
        "--sandbox",
        "read-only",
        "--skip-git-repo-check",
        "--json",
    ]);
    if model != "auto" {
        command.args(["--model", model]);
    }
    if think_mode != "auto" {
        command.args(["-c", &format!("model_reasoning_effort={think_mode}")]);
    }
    // No Web-supplied arguments, shell, write approvals or working directory.
    command.arg("-");
    command
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
    Ok(command)
}

fn spawn_codex(
    model: &str,
    think_mode: &str,
    cwd: Option<&std::path::Path>,
) -> Result<std::process::Child> {
    // Prefer the user's PATH, and on macOS fall back to existing Homebrew
    // launchers for Chrome's GUI environment. No installation is performed.
    #[cfg(target_os = "macos")]
    let candidates = ["codex", "/opt/homebrew/bin/codex", "/usr/local/bin/codex"];
    #[cfg(not(target_os = "macos"))]
    let candidates = ["codex"];

    for candidate in candidates {
        let mut command = codex_command(candidate, model, think_mode)?;
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        match command.spawn() {
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
    run_codex_web(prompt, "auto", "auto")
}

/// The same bounded, read-only Codex CLI task, with documented V2 options.
/// Returning a spawn receipt is not proof of a completed turn.
pub fn run_codex_web(prompt: &str, model: &str, think_mode: &str) -> Result<Value> {
    run_codex_in(prompt, model, think_mode, None)
}

pub(crate) fn run_codex_in(
    prompt: &str,
    model: &str,
    think_mode: &str,
    cwd: Option<&std::path::Path>,
) -> Result<Value> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT {
        bail!("agent prompt must contain 1..65536 bytes of text");
    }

    let mut child = spawn_codex(model, think_mode, cwd)?;

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

/// The user chooses one *already configured* Pi model. Local Bridge does
/// not own API keys, manage provider routes, or convert Web Chat into an API.
/// V1 only returns model text; there are deliberately no filesystem/shell tools.
fn pi_id(value: &str, allow_slash: bool) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('-')
        && !value.contains("..")
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'-' | b'_' | b'.' | b':')
                || (allow_slash && b == b'/')
        })
}

fn pi_command(provider: &str, model: &str) -> Result<Command> {
    if !pi_id(provider, false) || !pi_id(model, true) {
        bail!("invalid Pi provider or model ID");
    }
    let mut command = Command::new("pi");
    #[cfg(target_os = "macos")]
    {
        // Finder/Chrome GUI processes normally omit Homebrew from PATH.
        use std::path::PathBuf;
        let raw = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&raw).collect();
        for name in ["/opt/homebrew/bin", "/usr/local/bin"] {
            let dir = PathBuf::from(name);
            if dir.is_dir() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        if let Ok(paths) = std::env::join_paths(dirs) {
            command.env("PATH", paths);
        }
    }
    command.args([
        "--print",
        "--no-session",
        "--no-tools",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
        "--no-approve",
        "--provider",
        provider,
        "--model",
        model,
    ]);
    // Piped stdin avoids turning user text into Pi @file command-line arguments.
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(command)
}

/// One explicit, text-only Pi task. Pi itself resolves the chosen provider's
/// API key or OAuth from its own login/configuration. Missing auth is an error,
/// not an automatic switch to a different provider or a web-chat quota.
pub fn run_pi(provider: &str, model: &str, prompt: &str) -> Result<Value> {
    run_pi_command(pi_command(provider, model)?, provider, model, prompt)
}

/// One browser-approved, text-only Pi run using the user's own installed
/// model credentials. Unlike the legacy fixed CLI shortcut, user-installed
/// model *extensions* remain discoverable (e.g. a configured model proxy), but
/// all Pi tool calls, MCP, project context and session persistence are disabled.
fn pi_web_command(model: &str, think_mode: &str) -> Result<Command> {
    if model != "auto" && !pi_id(model, true) {
        bail!("invalid Pi model");
    }
    if think_mode != "auto" && !["off", "low", "medium", "high", "xhigh"].contains(&think_mode) {
        bail!("unsupported Pi think mode");
    }
    // Finder-launched Chrome Hosts may not inherit Homebrew's PATH.
    // Prefer a known installed executable; user profile/extensions stay Pi-owned.
    #[cfg(target_os = "macos")]
    let program = ["/opt/homebrew/bin/pi", "/usr/local/bin/pi"]
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
        .unwrap_or("pi");
    #[cfg(not(target_os = "macos"))]
    let program = "pi";
    let mut command = Command::new(program);
    #[cfg(target_os = "macos")]
    {
        use std::path::PathBuf;
        let raw = std::env::var_os("PATH").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::split_paths(&raw).collect();
        for name in ["/opt/homebrew/bin", "/usr/local/bin"] {
            let dir = PathBuf::from(name);
            if dir.is_dir() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        if let Ok(paths) = std::env::join_paths(dirs) {
            command.env("PATH", paths);
        }
    }
    command.args([
        "--print",
        "--no-session",
        "--no-tools",
        "--no-skills",
        "--no-prompt-templates",
        "--no-context-files",
    ]);
    if model != "auto" {
        command.args(["--model", model]);
    }
    if think_mode != "auto" {
        command.args(["--thinking", think_mode]);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    Ok(command)
}
pub fn run_pi_web(prompt: &str, model: &str, think_mode: &str) -> Result<Value> {
    run_pi_command(pi_web_command(model, think_mode)?, "pi", model, prompt)
}

pub(crate) fn run_pi_check(
    prompt: &str,
    model: &str,
    think_mode: &str,
    cwd: &std::path::Path,
) -> Result<Value> {
    let mut command = pi_web_command(model, think_mode)?;
    command.current_dir(cwd).arg("--no-mcp").arg("--no-approve");
    run_pi_command(command, "pi", model, prompt)
}

/// Enterprise app provides one already-authenticated compatible model session.
/// No per-user Pi model/account setup, global Pi changes or Bridge model router.
pub fn run_pi_enterprise(prompt: &str) -> Result<Value> {
    let binding = pi_enterprise::EnterprisePiSession::from_environment()?;
    let mut command = pi_command(pi_enterprise::PROVIDER, &binding.model)?;
    command.env("PI_CODING_AGENT_DIR", &binding.agent_dir);
    // Skip remote Pi catalog/config updates; this still calls the enterprise API.
    command.env("PI_OFFLINE", "1");
    let mut result = run_pi_command(command, pi_enterprise::PROVIDER, &binding.model, prompt)?;
    result["model_source"] = json!("enterprise_session");
    Ok(result)
}

/// Trusted Web caller's enterprise backend supplies a short-lived bearer.
/// A Web page cannot call this directly: the registered Chrome extension must
/// verify its own sender origin and a separate site-specific AI grant first.
pub fn run_pi_enterprise_ticket(
    base_url: &str,
    model: &str,
    token: &str,
    prompt: &str,
) -> Result<Value> {
    let binding = pi_enterprise::EnterprisePiSession::from_values(base_url, model, token)?;
    let mut command = pi_command(pi_enterprise::PROVIDER, &binding.model)?;
    command
        .env("PI_CODING_AGENT_DIR", &binding.agent_dir)
        .env(pi_enterprise::TOKEN_ENV, token)
        .env("PI_OFFLINE", "1");
    let mut result = run_pi_command(command, pi_enterprise::PROVIDER, &binding.model, prompt)?;
    result["model_source"] = json!("enterprise_session");
    Ok(result)
}

fn run_pi_command(
    mut command: Command,
    provider: &str,
    model: &str,
    prompt: &str,
) -> Result<Value> {
    if prompt.trim().is_empty() || prompt.len() > MAX_PROMPT {
        bail!("Pi prompt must contain 1..65536 bytes of text");
    }
    let mut child = command
        .spawn()
        .context("Pi CLI unavailable, or enterprise model session not available")?;
    let stdout = child.stdout.take().context("Pi stdout unavailable")?;
    let reader = thread::spawn(move || -> io::Result<Option<Vec<u8>>> {
        let mut output = Vec::new();
        let mut stream = stdout;
        let mut buffer = [0_u8; 8192];
        let mut too_large = false;
        loop {
            let n = stream.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            if !too_large {
                if output.len() + n > MAX_EVENTS {
                    output.clear();
                    too_large = true;
                } else {
                    output.extend_from_slice(&buffer[..n]);
                }
            }
        }
        Ok((!too_large).then_some(output))
    });
    let sent = child
        .stdin
        .take()
        .context("Pi stdin unavailable")?
        .write_all(prompt.as_bytes());
    if sent.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader.join();
        bail!("Pi could not accept the prompt");
    }
    let deadline = Instant::now() + RUN_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().context("Pi process status unavailable")? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            bail!("Pi timed out; result not confirmed and no automatic retry was made");
        }
        thread::sleep(Duration::from_millis(100));
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Pi output reader failed"))??
        .context("Pi response exceeded size limit")?;
    if !status.success() {
        bail!("Pi did not complete; check the selected model and Pi's own /login or API key configuration");
    }
    let text = std::str::from_utf8(&bytes).context("Pi output was not UTF-8")?;
    if text.trim().is_empty() {
        bail!("Pi exited without a confirmed text response");
    }
    Ok(json!({
        "source":"local-cli",
        "owner_module":"ai.pi",
        "provider":provider,
        "model":model,
        "status":"completed",
        "result":text.trim(),
        "tools_enabled":false,
        "session_persisted":false,
        "persisted_by_bridge":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_gui_codex_child_path_can_find_homebrew_node() {
        let command = codex_command("/opt/homebrew/bin/codex", "auto", "auto").unwrap();
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
    fn pi_fallback_requires_explicit_safe_model_and_disables_local_tools() {
        let command = pi_command("deepseek", "deepseek-flash").unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        for arg in [
            "--print",
            "--no-tools",
            "--no-session",
            "--no-extensions",
            "--no-context-files",
            "--provider",
            "--model",
        ] {
            assert!(args.iter().any(|a| a == arg), "missing Pi option {arg}");
        }
        assert!(!args.iter().any(|a| a == "--api-key"));
        assert!(pi_command("--malicious", "model").is_err());
        assert!(pi_command("deepseek", "--dangerous").is_err());
        assert!(pi_command("deepseek", "../secret").is_err());
        assert!(pi_command("deepseek", "deepseek-v4-pro").is_ok());
    }

    #[test]
    fn web_cli_options_keep_existing_tools_bounded() {
        let codex = codex_command("codex", "gpt-6", "high").unwrap();
        let args: Vec<_> = codex
            .get_args()
            .map(|v| v.to_string_lossy().to_string())
            .collect();
        assert!(args.windows(2).any(|s| s == ["--model", "gpt-6"]));
        assert!(args
            .windows(2)
            .any(|s| s == ["-c", "model_reasoning_effort=high"]));
        assert!(codex_command("codex", "auto", "off").is_err());
        let pi = pi_web_command("9router/m3cn/MiniMax-M2.7", "medium").unwrap();
        let args: Vec<_> = pi
            .get_args()
            .map(|v| v.to_string_lossy().to_string())
            .collect();
        assert!(args.iter().any(|s| s == "--no-tools"));
        assert!(!args.iter().any(|s| s == "--no-extensions"));
        assert!(args.windows(2).any(|s| s == ["--thinking", "medium"]));
        assert!(pi_web_command("auto", "max").is_err());
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
