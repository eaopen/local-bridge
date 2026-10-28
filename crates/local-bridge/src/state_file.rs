//! Small OS-user-local atomic JSON updates, based on Agent2's private-store
//! semantics. Only used for Bridge's own low-frequency Web grants; never
//! applied to CloudFile's existing Journal, device keys or working copies.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
const LOCK_WAIT: Duration = Duration::from_secs(3);

fn private_parent(path: &Path) -> Result<()> {
    let parent = path.parent().context("local state file has no parent")?;
    if let Ok(metadata) = fs::symlink_metadata(parent) {
        if metadata.file_type().is_symlink() {
            bail!("local state directory cannot be a symlink");
        }
    }
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn new_private_file(path: &Path) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

/// Cross-process lock for a short read-modify-write transaction. Rust 1.90's
/// std File lock is released by the OS if the CLI crashes, unlike a permanent
/// create_new lock file. The lock file itself is intentionally never deleted.
pub fn with_lock<T>(path: &Path, work: impl FnOnce() -> Result<T>) -> Result<T> {
    private_parent(path)?;
    let lock_path = path.with_extension("lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.file_type().is_file()) {
        bail!("invalid local grant lock");
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(&lock_path)?;
    let started = Instant::now();
    loop {
        match lock.try_lock().map_err(std::io::Error::from) {
            Ok(()) => break,
            Err(error) if error.kind() == ErrorKind::WouldBlock && started.elapsed() < LOCK_WAIT => {
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(error) => return Err(error).context("another local grant update is active"),
        }
    }
    // File::lock/try_lock is advisory on supported platforms; all Bridge grant
    // writers use this same lock. Readers see an atomic complete JSON file.
    work()
}

fn busy(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
        || error.kind() == ErrorKind::PermissionDenied
}

/// Serialize into a unique private file, fsync, then atomically replace the
/// original. A stale *.json.tmp from older Bridge builds is safely ignored.
/// On failed replacement the original file remains readable and authoritative.
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let content = serde_json::to_vec(value)?;
    private_parent(path)?;
    if fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()) {
        bail!("invalid local state target");
    }
    let parent = path.parent().context("local state target missing parent")?;
    let time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .context("local state target missing file name")?
        .to_string_lossy();
    let tmp = parent.join(format!(".{name}.{}-{time}-{unique}.tmp", std::process::id()));
    let mut file = new_private_file(&tmp)?;
    let result = (|| -> Result<()> {
        file.write_all(&content)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }

    let mut retries = 0_u64;
    loop {
        match fs::rename(&tmp, path) {
            Ok(()) => break,
            Err(error) if cfg!(windows) && busy(&error) && retries < 6 => {
                retries += 1;
                std::thread::sleep(Duration::from_millis(40 * retries));
            }
            Err(error) => {
                let _ = fs::remove_file(&tmp);
                return Err(error).context("cannot atomically replace local grant file");
            }
        }
    }
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn sandbox() -> std::path::PathBuf {
        let value = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "local-bridge-state-test-{}-{value}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn updates_ignore_stale_legacy_temp_and_keep_original_json_shape() {
        let root = sandbox();
        let path = root.join("web-v2-agent-grants.json");
        let legacy_tmp = root.join("web-v2-agent-grants.json.tmp");
        fs::write(&legacy_tmp, b"interrupted older write").unwrap();
        write_json(&path, &json!([{"origin":"https://example.test","agent":"pi"}])).unwrap();
        write_json(&path, &json!([{"origin":"https://example.test","agent":"codex"}])).unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw[0]["agent"], "codex");
        assert_eq!(fs::read(legacy_tmp).unwrap(), b"interrupted older write");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_lock_serializes_concurrent_grant_mutations() {
        let root = sandbox();
        let path = root.join("counter.json");
        write_json(&path, &json!({"value":0})).unwrap();
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let target = path.clone();
            tasks.push(std::thread::spawn(move || {
                for _ in 0..8 {
                    with_lock(&target, || {
                        let mut state: Value =
                            serde_json::from_slice(&fs::read(&target)?)?;
                        state["value"] = json!(state["value"].as_u64().unwrap() + 1);
                        write_json(&target, &state)
                    }).unwrap();
                }
            }));
        }
        for handle in tasks {
            handle.join().unwrap();
        }
        let state: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(state["value"], 32);
        fs::remove_dir_all(root).unwrap();
    }
}
