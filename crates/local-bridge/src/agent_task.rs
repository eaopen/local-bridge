//! Bounded, OS-user-local task records for the existing Codex and Pi text
//! CLI adapters. One detached worker survives short Native Messaging calls.
//! WorkBuddy retains its existing ACP worker and event store.
use crate::{agent, agent_grants, inbox};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const TTL_MS: u64 = 30 * 60_000;
const UNKNOWN_AFTER_MS: u64 = 195_000; // CLI timeout is 180 seconds.
const MAX_EVENTS: u64 = 512_000;
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
struct Meta {
    id: String,
    origin: String,
    client_key: String,
    agent: String,
    created: u64,
    model: String,
    think_mode: String,
}
#[derive(Serialize, Deserialize)]
struct Request {
    prompt: String,
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}
fn root() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("agent-v2-tasks"))
}
fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn private_write(path: &Path, data: &[u8]) -> Result<()> {
    let mut opts = OpenOptions::new();
    opts.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}
fn metadata(dir: &Path) -> Result<Meta> {
    let file = dir.join("meta.json");
    if fs::metadata(&file)?.len() > 4096 {
        bail!("invalid Agent task metadata size");
    }
    Ok(serde_json::from_slice(&fs::read(file)?)?)
}
fn events(dir: &Path) -> Result<Vec<Value>> {
    let file = dir.join("events.jsonl");
    if !file.exists() {
        return Ok(Vec::new());
    }
    if fs::metadata(&file)?.len() > MAX_EVENTS {
        bail!("Agent task events exceeded limit");
    }
    let bytes = fs::read(file)?;
    let mut result = Vec::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        if !line.ends_with(b"\n") {
            break; // A concurrent append is not yet a complete event.
        }
        let event: Value = serde_json::from_slice(&line[..line.len() - 1])?;
        if event["seq"].as_u64() != Some(result.len() as u64 + 1) {
            bail!("invalid Agent event sequence");
        }
        result.push(event);
    }
    Ok(result)
}
fn append(dir: &Path, seq: u64, mut value: Value) -> Result<()> {
    value["seq"] = json!(seq);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("events.jsonl"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let bytes = serde_json::to_vec(&value)?;
    if bytes.len() > 110_000 {
        bail!("Agent event too large");
    }
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
fn state(items: &[Value], age: u64) -> &'static str {
    for event in items.iter().rev() {
        match event["type"].as_str() {
            Some("task.completed") => return "completed",
            Some("task.failed") => return "failed",
            Some("task.unknown") => return "unknown",
            Some("task.started") => break,
            _ => {}
        }
    }
    if age > UNKNOWN_AFTER_MS {
        "unknown"
    } else if items.is_empty() {
        "queued"
    } else {
        "running"
    }
}
/// Lookup is shared by HTTP and short-lived Native Host callers. The caller
/// holds agent_options' cross-process start lock; never launch a second worker
/// for a reused client_key.
pub fn find(origin: &str, client_key: &str) -> Result<Option<(String, String)>> {
    let base = root()?;
    if !base.exists() {
        return Ok(None);
    }
    let time = now()?;
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(meta) = metadata(&entry.path()) else {
            continue;
        };
        if time.saturating_sub(meta.created) <= TTL_MS
            && meta.origin == origin
            && meta.client_key == client_key
        {
            return Ok(Some((meta.id, meta.agent)));
        }
    }
    Ok(None)
}
pub fn start(
    origin: &str,
    client_key: &str,
    provider: &str,
    prompt: &str,
    model: &str,
    think_mode: &str,
) -> Result<Value> {
    if !["codex", "pi"].contains(&provider)
        || !agent_grants::allowed(origin, provider)?
        || prompt.trim().is_empty()
        || prompt.len() > 12_000
    {
        bail!("Agent task not authorized or prompt invalid");
    }
    let base = root()?;
    private_dir(&base)?;
    let time = now()?;
    let mut count = 0;
    for entry in fs::read_dir(&base)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Ok(meta) = metadata(&entry.path()) else {
            continue;
        };
        if time.saturating_sub(meta.created) > TTL_MS {
            let _ = fs::remove_dir_all(entry.path());
        } else {
            count += 1;
        }
    }
    if count >= 40 {
        bail!("too many active/recent Agent tasks");
    }
    let id = format!(
        "{provider}-{time:x}-{:x}-{:x}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = base.join(&id);
    private_dir(&dir)?;
    let meta = Meta {
        id: id.clone(),
        origin: origin.into(),
        client_key: client_key.into(),
        agent: provider.into(),
        created: time,
        model: model.into(),
        think_mode: think_mode.into(),
    };
    private_write(&dir.join("meta.json"), &serde_json::to_vec(&meta)?)?;
    private_write(
        &dir.join("request.json"),
        &serde_json::to_vec(&Request {
            prompt: prompt.into(),
        })?,
    )?;

    // The current binary may be either local-bridge or local-bridge-native-host.
    // Both expose this *internal* worker entry. No Shell, no Web-supplied path.
    let worker = Command::new(std::env::current_exe()?)
        .arg("--internal-agent-v2-worker")
        .arg(&id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if worker.is_err() {
        // Persist a terminal record: a lost start response must never re-run.
        append(
            &dir,
            1,
            json!({"type":"task.failed","code":"worker_spawn_failed"}),
        )?;
    }
    Ok(json!({"task_id":id,"status":"accepted","reused":false,"provider":provider}))
}
pub fn poll(origin: &str, id: &str, cursor: u64) -> Result<Value> {
    if !safe_id(id) || !(id.starts_with("codex-") || id.starts_with("pi-")) {
        bail!("invalid Agent task ID");
    }
    let dir = root()?.join(id);
    let meta = metadata(&dir)?;
    if meta.id != id || meta.origin != origin || !agent_grants::allowed(origin, &meta.agent)? {
        bail!("Agent task origin/grant mismatch");
    }
    let age = now()?.saturating_sub(meta.created);
    if age > TTL_MS {
        bail!("Agent task expired");
    }
    let all = events(&dir)?;
    let last = all.last().and_then(|e| e["seq"].as_u64()).unwrap_or(0);
    if cursor > last {
        bail!("invalid Agent cursor");
    }
    let page: Vec<_> = all
        .iter()
        .filter(|e| e["seq"].as_u64().unwrap_or(0) > cursor)
        .take(40)
        .cloned()
        .collect();
    let next = page
        .last()
        .and_then(|e| e["seq"].as_u64())
        .unwrap_or(cursor);
    let status = state(&all, age);
    Ok(json!({
        "task_id":id,"status":status,"events":page,"next_seq":next,
        "has_more":last > next,
        "terminal":matches!(status,"completed"|"failed"|"unknown")
    }))
}
/// Invoked only as a private subprocess, never from an HTTP request.
pub fn worker(id: &str) -> Result<()> {
    if !safe_id(id) || !(id.starts_with("codex-") || id.starts_with("pi-")) {
        bail!("invalid internal worker task ID");
    }
    let dir = root()?.join(id);
    let meta = metadata(&dir)?;
    if meta.id != id || !agent_grants::allowed(&meta.origin, &meta.agent)? {
        bail!("revoked Agent task or invalid metadata");
    }
    let request: Request = serde_json::from_slice(&fs::read(dir.join("request.json"))?)?;
    append(&dir, 1, json!({"type":"task.started"}))?;
    let result = match meta.agent.as_str() {
        "codex" => agent::run_codex_web(&request.prompt, &meta.model, &meta.think_mode),
        "pi" => agent::run_pi_web(&request.prompt, &meta.model, &meta.think_mode),
        _ => bail!("unsupported worker Agent"),
    };
    let _ = fs::remove_file(dir.join("request.json"));
    match result {
        Ok(value) if value["status"] == "completed" => {
            let text = value["result"]
                .as_str()
                .context("Agent completed without text")?;
            if text.trim().is_empty() {
                bail!("Agent completed with empty output");
            }
            append(
                &dir,
                2,
                json!({"type":"message.delta","message_id":"result","text":text}),
            )?;
            append(
                &dir,
                3,
                json!({"type":"message.final","message_id":"result"}),
            )?;
            append(
                &dir,
                4,
                json!({"type":"task.completed","stop_reason":"cli_completed"}),
            )?;
        }
        _ => append(
            &dir,
            2,
            json!({"type":"task.failed","code":"agent_execution_failed"}),
        )?,
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_is_not_inferred_from_message_final() {
        let mut events = vec![
            json!({"type":"task.started"}),
            json!({"type":"message.final"}),
        ];
        assert_eq!(state(&events, 100), "running");
        events.push(json!({"type":"task.completed"}));
        assert_eq!(state(&events, 100), "completed");
        assert_eq!(state(&events[..2], UNKNOWN_AFTER_MS + 1), "unknown");
    }
    #[test]
    fn ids_reject_path_traversal() {
        assert!(!safe_id("../task"));
        assert!(!safe_id("pi-1/../../a"));
    }
}
