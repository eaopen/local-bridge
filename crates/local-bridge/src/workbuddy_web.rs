//! WorkBuddy Web V2: one macOS native-account text-only job, cursor-based replies.
//! Browser AND OS-user grants are required. No tool calls, shell or auto retry.
use crate::{inbox, state_file};
#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{bail, Result};
use local_bridge_core::valid_origin;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
#[cfg(target_os = "macos")]
use std::fs::OpenOptions;
#[cfg(target_os = "macos")]
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const TTL: u64 = 30 * 60_000;
const UNKNOWN_AFTER: u64 = 195_000;
#[cfg(target_os = "macos")]
static COUNT: AtomicU64 = AtomicU64::new(0);
#[derive(Serialize, Deserialize)]
struct Meta {
    id: String,
    origin: String,
    client_key: String,
    created: u64,
}

fn timestamp() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}
fn root() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("workbuddy-web-v2"))
}
fn grant_file() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("workbuddy-web-v2-grants.json"))
}
fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}
#[cfg(target_os = "macos")]
fn private_dir(p: &Path) -> Result<()> {
    fs::create_dir_all(p)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn private_write(p: &Path, b: &[u8]) -> Result<()> {
    let mut o = OpenOptions::new();
    o.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(p)?;
    f.write_all(b)?;
    f.sync_all()?;
    Ok(())
}
fn read_grants() -> Result<Vec<String>> {
    let p = grant_file()?;
    if !p.exists() {
        return Ok(vec![]);
    }
    let info = fs::metadata(&p)?;
    if !info.is_file() || info.len() > 4096 {
        bail!("bad agent grants");
    }
    let ids: Vec<String> = serde_json::from_slice(&fs::read(p)?)?;
    if ids.len() > 32 || ids.iter().any(|x| !valid_origin(x)) {
        bail!("invalid agent grants");
    }
    Ok(ids)
}
fn save_grants(ids: &[String]) -> Result<()> {
    state_file::write_json(&grant_file()?, ids)
}

pub fn grant(origin: &str) -> Result<Value> {
    if !valid_origin(origin) {
        bail!("invalid agent origin");
    }
    state_file::with_lock(&grant_file()?, || {
        let mut ids = read_grants()?;
        if !ids.iter().any(|x| x == origin) {
            if ids.len() >= 32 {
                bail!("too many grants");
            }
            ids.push(origin.into());
            save_grants(&ids)?;
        }
        Ok(())
    })?;
    Ok(json!({"origin":origin,"provider":"workbuddy","granted":true,"mode":"text_only_no_tools"}))
}
pub fn revoke(origin: &str) -> Result<Value> {
    if !valid_origin(origin) {
        bail!("invalid agent origin");
    }
    state_file::with_lock(&grant_file()?, || {
        let mut ids = read_grants()?;
        ids.retain(|x| x != origin);
        save_grants(&ids)
    })?;
    Ok(json!({"origin":origin,"provider":"workbuddy","revoked":true}))
}
pub fn allowed(origin: &str) -> Result<bool> {
    if !valid_origin(origin) {
        bail!("invalid agent origin");
    }
    Ok(read_grants()?.iter().any(|x| x == origin))
}
pub fn installed() -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new("/Applications/WorkBuddy.app/Contents/Resources/app.asar.unpacked/cli/dist/codebuddy-lite-wb.mjs").is_file()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn capability(origin: &str) -> Result<Value> {
    let granted = allowed(origin)?;
    let installed = installed();
    Ok(
        json!({"provider":"workbuddy","installed":installed,"local_grant":granted,
        "ready":false,"auth_verified":false,"run_supported":cfg!(target_os="macos"),
        "task_mode":"text_only_no_tools"}),
    )
}
fn metadata(p: &Path) -> Result<Meta> {
    let f = p.join("meta.json");
    let m = fs::metadata(&f)?;
    if !m.is_file() || m.len() > 4096 {
        bail!("invalid task metadata");
    }
    Ok(serde_json::from_slice(&fs::read(f)?)?)
}
fn events(p: &Path) -> Result<Vec<Value>> {
    let f = p.join("events.jsonl");
    if !f.exists() {
        return Ok(vec![]);
    }
    let info = fs::metadata(&f)?;
    if !info.is_file() || info.len() > 512_000 {
        bail!("task event limit");
    }
    let data = fs::read(f)?;
    let mut out = Vec::new();
    for part in data.split_inclusive(|b| *b == b'\n') {
        if !part.ends_with(b"\n") {
            break;
        }
        let e: Value = serde_json::from_slice(&part[..part.len() - 1])?;
        if e["seq"].as_u64() != Some(out.len() as u64 + 1) {
            bail!("invalid task event sequence");
        }
        out.push(e);
    }
    Ok(out)
}
fn status(e: &[Value], age: u64) -> &'static str {
    for event in e.iter().rev() {
        match event["type"].as_str() {
            Some("task.completed") => return "completed",
            Some("task.failed") => return "failed",
            Some("task.unknown") => return "unknown",
            Some("task.needs_action") => return "needs_action",
            Some("task.started") => break,
            _ => {}
        }
    }
    if age > UNKNOWN_AFTER {
        "unknown"
    } else if e.is_empty() {
        "queued"
    } else {
        "running"
    }
}
// Serializes only the short "find existing client_key / create new task"
// transaction across separate Native Host and HTTP processes. Never keep this
// lock for the actual long-running ACP task.
#[cfg(target_os = "macos")]
struct StartLock(PathBuf);

#[cfg(target_os = "macos")]
impl StartLock {
    fn acquire(dir: &Path) -> Result<Self> {
        let path = dir.join(".start.lock");
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&path) {
            Ok(mut file) => {
                let lock = Self(path);
                file.write_all(b"workbuddy-start\\n")?;
                file.sync_all()?;
                Ok(lock)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let meta = fs::symlink_metadata(&path)?;
                if !meta.file_type().is_file() {
                    bail!("invalid task start lock");
                }
                if meta.modified()?.elapsed().unwrap_or_default()
                    > std::time::Duration::from_secs(60)
                {
                    // A crashed short-lived starter may leave a stale lock.
                    // Clear but fail closed; the user retries with the SAME
                    // client_key only after this operation has completed.
                    let _ = fs::remove_file(&path);
                    bail!("stale task start lock cleared; retry same client key");
                }
                bail!("WorkBuddy task submission already in progress")
            }
            Err(error) => Err(error).context("cannot lock WorkBuddy task submission"),
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for StartLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn assets(dir: &Path) -> Result<()> {
    private_write(
        &dir.join("worker.mjs"),
        include_str!("../../../scripts/workbuddy-web-v2/worker.mjs").as_bytes(),
    )?;
    private_write(
        &dir.join("workbuddyBroker.ts"),
        include_str!("../../../scripts/workbuddy-native-validation/workbuddyBroker.ts").as_bytes(),
    )?;
    private_write(
        &dir.join("workbuddyModels.ts"),
        include_str!("../../../scripts/workbuddy-native-validation/workbuddyModels.ts").as_bytes(),
    )?;
    Ok(())
}
#[cfg(target_os = "macos")]
fn node_path() -> Result<String> {
    for s in ["/opt/homebrew/bin/node", "/usr/local/bin/node", "node"] {
        if !s.starts_with('/') || Path::new(s).is_file() {
            return Ok(s.to_owned());
        }
    }
    bail!("Node 24+ unavailable")
}

/// Existing WorkBuddy worker metadata is the owner of task status; this
/// read-only local projection never starts/queries WorkBuddy or reveals prompts.
fn summaries_from(base: &Path, now: u64) -> Result<Vec<Value>> {
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut scanned = 0;
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        scanned += 1;
        if scanned > 256 {
            bail!("too many WorkBuddy task directories for summary");
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        if !safe_id(&id) || !id.starts_with("wb-") {
            continue;
        }
        let Ok(meta) = metadata(&entry.path()) else {
            continue;
        };
        if meta.id != id {
            bail!("invalid WorkBuddy task summary metadata");
        }
        let age = now.saturating_sub(meta.created);
        if age > TTL {
            continue;
        }
        result.push(json!({
            "task_id":id,
            "owner_module":"ai",
            "kind":"ai",
            "provider":"workbuddy",
            "title":"WorkBuddy task",
            "status":status(&events(&entry.path())?, age),
            "created_at_unix_ms":meta.created,
            "available_actions":[],
        }));
    }
    result.sort_by(|a, b| {
        b["created_at_unix_ms"]
            .as_u64()
            .cmp(&a["created_at_unix_ms"].as_u64())
    });
    result.truncate(40);
    Ok(result)
}
pub fn summaries() -> Result<Vec<Value>> {
    summaries_from(&root()?, timestamp()?)
}

/// Existing WorkBuddy tasks participate in the common V2 client_key
/// idempotency check before Bridge chooses another Agent.
pub fn find(origin: &str, client_key: &str) -> Result<Option<String>> {
    let base = root()?;
    if !base.exists() {
        return Ok(None);
    }
    let current = timestamp()?;
    for item in fs::read_dir(base)? {
        let item = item?;
        if !item.file_type()?.is_dir() {
            continue;
        }
        let Ok(meta) = metadata(&item.path()) else {
            continue;
        };
        if meta.origin == origin
            && meta.client_key == client_key
            && current.saturating_sub(meta.created) <= TTL
        {
            return Ok(Some(meta.id));
        }
    }
    Ok(None)
}

pub fn start(origin: &str, client_key: &str, prompt: &str) -> Result<Value> {
    if !allowed(origin)? {
        bail!("WorkBuddy not allowed on this origin");
    }
    if !safe_id(client_key)
        || prompt.trim().is_empty()
        || prompt.len() > 12000
        || prompt.contains('\0')
    {
        bail!("invalid WorkBuddy text task");
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (client_key, prompt);
        bail!("WorkBuddy desktop worker only validated on macOS");
    }
    #[cfg(target_os = "macos")]
    {
        if !Path::new("/Applications/WorkBuddy.app/Contents/Resources/app.asar.unpacked/cli/dist/codebuddy-lite-wb.mjs").is_file(){
            bail!("WorkBuddy Desktop not installed");
        }
        let base = root()?;
        private_dir(&base)?;
        let _start_lock = StartLock::acquire(&base)?;
        let now = timestamp()?;
        let mut count = 0;
        for item in fs::read_dir(&base)? {
            let item = item?;
            if !item.file_type()?.is_dir() {
                continue;
            }
            let Ok(meta) = metadata(&item.path()) else {
                continue;
            };
            if now.saturating_sub(meta.created) > TTL {
                let _ = fs::remove_dir_all(item.path());
                continue;
            }
            count += 1;
            if meta.origin == origin && meta.client_key == client_key {
                return Ok(json!({"task_id":meta.id,"status":"accepted","reused":true}));
            }
            if now.saturating_sub(meta.created) < UNKNOWN_AFTER
                && matches!(
                    status(&events(&item.path())?, now.saturating_sub(meta.created)),
                    "queued" | "running" | "needs_action"
                )
            {
                bail!("WorkBuddy task already running");
            }
        }
        if count >= 40 {
            bail!("too many recent tasks");
        }
        let id = format!(
            "wb-{now:x}-{:x}-{:x}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = base.join(&id);
        private_dir(&dir)?;
        let meta = Meta {
            id: id.clone(),
            origin: origin.into(),
            client_key: client_key.into(),
            created: now,
        };
        private_write(&dir.join("meta.json"), &serde_json::to_vec(&meta)?)?;
        assets(&dir)?;
        let mut child = Command::new(node_path()?)
            .arg("--experimental-strip-types")
            .arg(dir.join("worker.mjs"))
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("cannot start WorkBuddy worker")?;
        let body = serde_json::to_vec(&json!({"dir":dir,"prompt":prompt}))?;
        if child
            .stdin
            .take()
            .is_none_or(|mut io| io.write_all(&body).is_err())
        {
            let _ = child.kill();
            bail!("WorkBuddy worker input unavailable");
        }
        Ok(json!({"task_id":id,"status":"accepted","reused":false,"provider":"workbuddy"}))
    }
}
pub fn poll(origin: &str, task_id: &str, cursor: u64) -> Result<Value> {
    if !allowed(origin)? {
        bail!("WorkBuddy site grant revoked");
    }
    if !safe_id(task_id) || !task_id.starts_with("wb-") {
        bail!("invalid task id");
    }
    let dir = root()?.join(task_id);
    let meta = metadata(&dir)?;
    if meta.id != task_id || meta.origin != origin {
        bail!("task origin mismatch");
    }
    let age = timestamp()?.saturating_sub(meta.created);
    if age > TTL {
        bail!("task expired");
    }
    let all = events(&dir)?;
    let last = all.last().and_then(|x| x["seq"].as_u64()).unwrap_or(0);
    if cursor > last {
        bail!("invalid task cursor");
    }
    let page: Vec<Value> = all
        .iter()
        .filter(|x| x["seq"].as_u64().unwrap_or(0) > cursor)
        .take(40)
        .cloned()
        .collect();
    let next = page
        .last()
        .and_then(|x| x["seq"].as_u64())
        .unwrap_or(cursor);
    let state = status(&all, age);
    Ok(
        json!({"task_id":task_id,"status":state,"events":page,"next_seq":next,
        "has_more":last>next,"terminal":matches!(state,"completed"|"failed"|"unknown")}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn concurrent_submission_is_rejected_until_original_start_releases_lock() {
        let path = std::env::temp_dir().join(format!(
            "local-bridge-workbuddy-lock-{}-{}",
            std::process::id(),
            timestamp().unwrap()
        ));
        private_dir(&path).unwrap();
        let first = StartLock::acquire(&path).unwrap();
        assert!(StartLock::acquire(&path).is_err());
        drop(first);
        let second = StartLock::acquire(&path).unwrap();
        drop(second);
        fs::remove_dir_all(&path).unwrap();
    }

    #[test]
    fn read_only_summary_does_not_count_message_final_as_task_completion() {
        let home = std::env::temp_dir().join(format!(
            "local-bridge-wb-summary-{}-{}",
            std::process::id(),
            timestamp().unwrap()
        ));
        let id = "wb-1234-a-b";
        let dir = home.join(id);
        fs::create_dir_all(&dir).unwrap();
        let meta = Meta {
            id: id.into(),
            origin: "https://private.example".into(),
            client_key: "private-key".into(),
            created: timestamp().unwrap(),
        };
        fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
        fs::write(
            dir.join("events.jsonl"),
            b"{\"seq\":1,\"type\":\"task.started\"}\n{\"seq\":2,\"type\":\"message.final\",\"text\":\"private\"}\n"
        ).unwrap();
        let result = summaries_from(&home, meta.created + 20).unwrap();
        assert_eq!(result[0]["status"], "running");
        assert!(!result[0].to_string().contains("private"));
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn multiple_messages_are_not_completion() {
        let mut items = vec![
            json!({"seq":1,"type":"task.started"}),
            json!({"seq":2,"type":"message.delta","text":"partial"}),
            json!({"seq":3,"type":"message.final"}),
        ];
        assert_eq!(status(&items, 100), "running");
        items.push(json!({"seq":4,"type":"task.completed"}));
        assert_eq!(status(&items, 100), "completed");
        assert_eq!(status(&items[..3], UNKNOWN_AFTER + 1), "unknown");
    }
    #[test]
    fn origin_and_ids_reject_traversal() {
        assert!(!safe_id("../run"));
        assert!(!valid_origin("https://example.org/path"));
    }
}
