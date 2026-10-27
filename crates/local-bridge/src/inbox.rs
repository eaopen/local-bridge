//! A small, explicit, per-OS-user inbox for contexts voluntarily shared
//! from the Chrome extension. No cloud sync, account model or task database.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn data_root() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        return env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(|v| PathBuf::from(v).join("LocalBridge"))
            .context("LOCALAPPDATA is not set");
    }
    #[cfg(target_os = "macos")]
    {
        return env::var_os("HOME")
            .map(|v| PathBuf::from(v).join("Library/Application Support/LocalBridge"))
            .context("HOME is not set");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                return Ok(path.join("local-bridge"));
            }
        }
        env::var_os("HOME")
            .map(|v| PathBuf::from(v).join(".local/share/local-bridge"))
            .context("HOME is not set")
    }
}

fn inbox_dir() -> Result<PathBuf> {
    Ok(data_root()?.join("inbox"))
}

fn valid_id(id: &str) -> bool {
    id.len() >= 12
        && id.len() <= 96
        && id
            .strip_prefix("ctx-")
            .is_some_and(|tail| tail.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
}

fn ensure_private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).context("cannot create local inbox directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .context("cannot set local inbox permissions")?;
    }
    Ok(())
}

/// The caller supplies already validated, explicitly user-shared context.
/// Chrome's original login, cookies and API tokens are not persisted.
pub fn save(context: Value) -> Result<String> {
    save_to(&inbox_dir()?, context)
}

fn save_to(dir: &Path, context: Value) -> Result<String> {
    ensure_private_dir(dir)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("invalid system clock")?
        .as_nanos();
    for index in 0_u32..16 {
        let id = format!("ctx-{nanos:x}-{:x}-{index:x}", std::process::id());
        let path = dir.join(format!("{id}.json"));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = match opts.open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).context("cannot create local inbox item"),
        };
        let doc = json!({
            "schema_version": 1,
            "id": id,
            "created_at_unix_ms": (nanos / 1_000_000) as u64,
            "source": "browser-extension",
            "context": context
        });
        let write = (|| -> Result<()> {
            serde_json::to_writer(&mut file, &doc)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(e) = write {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(e).context("cannot write local inbox item");
        }
        return Ok(id);
    }
    bail!("cannot allocate a unique inbox item")
}

pub fn list() -> Result<Value> {
    let dir = inbox_dir()?;
    if !dir.exists() {
        return Ok(json!({"items":[]}));
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let id = entry.file_name().to_string_lossy().to_string();
            if let Some(id) = id.strip_suffix(".json") {
                if valid_id(id) {
                    entries.push(id.to_string());
                }
            }
        }
    }
    entries.sort_unstable_by(|a, b| b.cmp(a));
    let mut items = Vec::new();
    for id in entries.into_iter().take(100) {
        let document = read(&id)?;
        items.push(json!({
            "id":id,
            "title":document["context"]["title"].as_str().unwrap_or(""),
            "created_at_unix_ms":document["created_at_unix_ms"]
        }));
    }
    Ok(json!({"items":items}))
}

pub fn read(id: &str) -> Result<Value> {
    if !valid_id(id) {
        bail!("invalid inbox item id");
    }
    let path = inbox_dir()?.join(format!("{id}.json"));
    let metadata = fs::metadata(&path).context("inbox item not found")?;
    if !metadata.is_file() || metadata.len() > 128 * 1024 {
        bail!("invalid inbox item");
    }
    let item: Value = serde_json::from_slice(&fs::read(path)?)?;
    if item["id"] != id {
        bail!("inbox item id mismatch");
    }
    Ok(item)
}

pub fn delete(id: &str) -> Result<Value> {
    if !valid_id(id) {
        bail!("invalid inbox item id");
    }
    fs::remove_file(inbox_dir()?.join(format!("{id}.json")))
        .context("inbox item not found")?;
    Ok(json!({"id":id,"deleted":true}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_minimal_context_in_per_user_directory() {
        let dir = env::temp_dir().join(format!(
            "local-bridge-inbox-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let id = save_to(&dir, json!({"title":"example","url":"","selection":"hello"})).unwrap();
        assert!(valid_id(&id));
        let stored: Value = serde_json::from_slice(
            &fs::read(dir.join(format!("{id}.json"))).unwrap()
        ).unwrap();
        assert_eq!(stored["context"]["selection"], "hello");
        assert_eq!(stored["source"], "browser-extension");
        assert!(!valid_id("../traversal"));
        fs::remove_dir_all(dir).unwrap();
    }
}
