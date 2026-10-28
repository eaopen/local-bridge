//! User-owned Chrome Native Host registration, independent of CloudFile's host.
//! Adapted from the proven Agent2 Host lifecycle: inspect, preserve one previous
//! manifest, install/repair, rollback, and remove only what Local Bridge owns.
//! No file-task, Journal, desktop, or OS-admin privileges are involved.
use crate::inbox;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const HOST_NAME: &str = "com.eaopen.browser_bridge";
const DESCRIPTION: &str = "Optional Local Bridge native provider for Local Tool Bridge";
const MAX_MANIFEST: u64 = 16 * 1024;

fn valid_extension_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b))
}

fn manifest_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        Ok(inbox::data_root()?.join("NativeMessagingHosts"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home)
            .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home).join(".config/google-chrome/NativeMessagingHosts"))
    }
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join(format!("{HOST_NAME}.json"))
}
fn previous_path(dir: &Path) -> PathBuf {
    dir.join(format!("{HOST_NAME}.previous.json"))
}
fn native_binary() -> Result<PathBuf> {
    let current = env::current_exe().context("cannot identify Local Bridge CLI")?;
    let name = if cfg!(windows) {
        "local-bridge-native-host.exe"
    } else {
        "local-bridge-native-host"
    };
    let path = current.with_file_name(name);
    if !path.is_file() {
        bail!("native host executable is missing beside Local Bridge CLI");
    }
    Ok(path.canonicalize()?)
}
fn make_manifest(binary: &Path, extension_id: &str) -> Result<Value> {
    if !valid_extension_id(extension_id) || !binary.is_absolute() {
        bail!("invalid Chrome extension ID or native host binary");
    }
    Ok(json!({
        "name":HOST_NAME,
        "description":DESCRIPTION,
        "path":binary.to_string_lossy(),
        "type":"stdio",
        "allowed_origins":[format!("chrome-extension://{extension_id}/")]
    }))
}
fn read_manifest(path: &Path) -> Result<Option<Value>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !meta.file_type().is_file() || meta.len() > MAX_MANIFEST {
        bail!("invalid Native Host manifest file");
    }
    Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
}
fn owner(value: &Value) -> bool {
    value["name"] == HOST_NAME
        && value["type"] == "stdio"
        && value["description"] == DESCRIPTION
        && value["path"]
            .as_str()
            .and_then(|s| Path::new(s).file_name())
            .is_some_and(|name| {
                name == if cfg!(windows) {
                    "local-bridge-native-host.exe"
                } else {
                    "local-bridge-native-host"
                }
            })
}
fn extension_id(value: &Value) -> Option<&str> {
    let origins = value["allowed_origins"].as_array()?;
    if origins.len() != 1 {
        return None;
    }
    let origin = origins[0].as_str()?;
    let id = origin
        .strip_prefix("chrome-extension://")?
        .strip_suffix('/')?;
    valid_extension_id(id).then_some(id)
}

/// Same-directory staging prevents an interrupted write from truncating Chrome's
/// live manifest. No other host name or data directory is ever modified.
fn write_manifest(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .context("missing native host manifest parent")?;
    fs::create_dir_all(dir)?;
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let tmp = dir.join(format!(".{HOST_NAME}.{}-{unique}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    let write = (|| -> Result<()> {
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = write {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    let mut attempts = 0;
    loop {
        match fs::rename(&tmp, path) {
            Ok(()) => break,
            Err(error)
                if cfg!(windows)
                    && (matches!(error.raw_os_error(), Some(5 | 32 | 33))
                        || error.kind() == std::io::ErrorKind::PermissionDenied)
                    && attempts < 6 =>
            {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(40 * attempts));
            }
            Err(error) => {
                let _ = fs::remove_file(&tmp);
                return Err(error).context("cannot activate Native Host manifest");
            }
        }
    }
    #[cfg(unix)]
    fs::File::open(dir)?.sync_all()?;
    Ok(())
}

fn status_at(dir: &Path) -> Result<Value> {
    let path = manifest_path(dir);
    let Some(current) = read_manifest(&path)? else {
        return Ok(json!({
            "host":HOST_NAME,"manifest_present":false,"owned":false,
            "binary_present":false,"extension_id":null,
            "state":"not_installed","browser_verified":false
        }));
    };
    let owned = owner(&current) && extension_id(&current).is_some();
    let binary_present = current["path"].as_str().is_some_and(|value| {
        let p = Path::new(value);
        p.is_absolute() && p.is_file()
    });
    let state = if !owned {
        "foreign_or_invalid_manifest"
    } else if !binary_present {
        "missing_native_binary"
    } else {
        "manifest_ready_browser_not_verified"
    };
    Ok(json!({
        "host":HOST_NAME, "manifest_present":true, "owned":owned,
        "binary_present":binary_present,
        "extension_id":if owned {extension_id(&current)} else {None},
        "state":state, "browser_verified":false
    }))
}
pub fn status() -> Result<Value> {
    status_at(&manifest_dir()?)
}

fn install_at(dir: &Path, binary: &Path, extension_id: &str, replace: bool) -> Result<Value> {
    let desired = make_manifest(binary, extension_id)?;
    let path = manifest_path(dir);
    let existing = read_manifest(&path)?;
    let mut backed_up = false;
    if let Some(previous) = &existing {
        if *previous != desired {
            if !replace {
                bail!("Native Host manifest differs; use --replace to back up and replace it");
            }
            // Preserve original bytes, including unknown fields and foreign
            // manifests, for an explicit rollback (no automatic takeover).
            write_manifest(&previous_path(dir), &fs::read(&path)?)?;
            backed_up = true;
        }
    }
    write_manifest(&path, &serde_json::to_vec_pretty(&desired)?)?;
    Ok(json!({
        "installed":true,"host":HOST_NAME,"extension_id":extension_id,
        "backed_up":backed_up,"manifest":path.to_string_lossy(),
        "restart_chrome_if_needed":true
    }))
}

#[cfg(windows)]
fn registry_key() -> String {
    format!("HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\{HOST_NAME}")
}
#[cfg(windows)]
fn registry_location() -> Result<Option<PathBuf>> {
    let result = std::process::Command::new("reg.exe")
        .args(["QUERY", &registry_key(), "/ve"])
        .output()
        .context("cannot inspect Native Host registry entry")?;
    if !result.status.success() {
        return Ok(None);
    }
    let report = String::from_utf8_lossy(&result.stdout);
    for line in report.lines() {
        if let Some((_, path)) = line.split_once("REG_SZ") {
            return Ok(Some(PathBuf::from(path.trim())));
        }
    }
    bail!("Native Host registry entry exists without a readable REG_SZ path")
}
#[cfg(windows)]
fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
}
#[cfg(windows)]
fn guard_registry_path(path: &Path) -> Result<()> {
    if registry_location()?.is_some_and(|existing| !same_path(&existing, path)) {
        bail!("a different Native Host registration owns the Windows registry entry");
    }
    Ok(())
}
#[cfg(windows)]
fn register_windows(path: &Path) -> Result<()> {
    let result = std::process::Command::new("reg.exe")
        .args(["ADD", &registry_key(), "/ve", "/t", "REG_SZ", "/d"])
        .arg(path)
        .arg("/f")
        .status()
        .context("cannot register Native Host")?;
    if !result.success() {
        bail!("Windows Native Host registration failed");
    }
    Ok(())
}
#[cfg(windows)]
fn unregister_windows() -> Result<()> {
    let result = std::process::Command::new("reg.exe")
        .args(["DELETE", &registry_key(), "/f"])
        .status()
        .context("cannot remove Native Host registration")?;
    if !result.success() {
        bail!("Windows Native Host registry removal failed");
    }
    Ok(())
}

pub fn install_host(extension_id: &str, replace: bool) -> Result<Value> {
    if !valid_extension_id(extension_id) {
        bail!("invalid Chrome extension id (expected 32 letters a-p)");
    }
    let binary = native_binary()?;
    let dir = manifest_dir()?;
    #[cfg(windows)]
    guard_registry_path(&manifest_path(&dir))?;
    let result = install_at(&dir, &binary, extension_id, replace)?;
    #[cfg(windows)]
    register_windows(&manifest_path(&dir))?;
    Ok(result)
}
fn rollback_at(dir: &Path) -> Result<Value> {
    let path = manifest_path(dir);
    if read_manifest(&path)?.is_some_and(|value| !owner(&value)) {
        bail!("a foreign Native Host manifest cannot be replaced by rollback");
    }
    let previous = previous_path(dir);
    let value = read_manifest(&previous)?.context("no previous Native Host manifest to restore")?;
    if value["name"] != HOST_NAME || value["type"] != "stdio" {
        bail!("previous Native Host manifest is invalid");
    }
    write_manifest(&path, &fs::read(&previous)?)?;
    Ok(json!({"restored":true,"host":HOST_NAME,"manifest":path.to_string_lossy()}))
}
pub fn rollback_host() -> Result<Value> {
    let dir = manifest_dir()?;
    #[cfg(windows)]
    guard_registry_path(&manifest_path(&dir))?;
    let result = rollback_at(&dir)?;
    #[cfg(windows)]
    register_windows(&manifest_path(&dir))?;
    Ok(result)
}
fn uninstall_at(dir: &Path) -> Result<Value> {
    let path = manifest_path(dir);
    let Some(value) = read_manifest(&path)? else {
        return Ok(json!({"removed":false,"host":HOST_NAME,"reason":"not_installed"}));
    };
    if !owner(&value) {
        bail!("refusing to remove Native Host manifest not owned by Local Bridge");
    }
    fs::remove_file(&path)?;
    // Keep a previous manifest for manual recovery; never touch other hosts.
    Ok(json!({"removed":true,"host":HOST_NAME}))
}
pub fn uninstall_host() -> Result<Value> {
    let dir = manifest_dir()?;
    let path = manifest_path(&dir);
    #[cfg(windows)]
    guard_registry_path(&path)?;
    let value = read_manifest(&path)?;
    if value.is_some_and(|v| !owner(&v)) {
        bail!("refusing to remove Native Host manifest not owned by Local Bridge");
    }
    #[cfg(windows)]
    if registry_location()?.is_some() {
        unregister_windows()?;
    }
    uninstall_at(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sandbox() -> PathBuf {
        let name = format!(
            "bridge-host-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = env::temp_dir().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn fake_binary(dir: &Path) -> PathBuf {
        let name = if cfg!(windows) {
            "local-bridge-native-host.exe"
        } else {
            "local-bridge-native-host"
        };
        let binary = dir.join(name);
        fs::write(&binary, b"synthetic test binary").unwrap();
        binary
    }
    const A: &str = "abcdefghijklmnopabcdefghijklmnop";
    const B: &str = "ponmlkjihgfedcbaponmlkjihgfedcba";

    #[test]
    fn register_replace_restore_and_remove_are_reversible() {
        let dir = sandbox();
        let binary = fake_binary(&dir);
        let a = install_at(&dir, &binary, A, false).unwrap();
        assert_eq!(a["installed"], true);
        assert_eq!(
            status_at(&dir).unwrap()["state"],
            "manifest_ready_browser_not_verified"
        );
        assert_eq!(status_at(&dir).unwrap()["extension_id"], A);
        assert!(!previous_path(&dir).exists());
        let again = install_at(&dir, &binary, A, false).unwrap();
        assert_eq!(again["backed_up"], false);
        assert!(install_at(&dir, &binary, B, false).is_err());
        let changed = install_at(&dir, &binary, B, true).unwrap();
        assert_eq!(changed["backed_up"], true);
        assert_eq!(status_at(&dir).unwrap()["extension_id"], B);
        rollback_at(&dir).unwrap();
        assert_eq!(status_at(&dir).unwrap()["extension_id"], A);
        assert_eq!(uninstall_at(&dir).unwrap()["removed"], true);
        assert_eq!(status_at(&dir).unwrap()["state"], "not_installed");
        assert_eq!(uninstall_at(&dir).unwrap()["removed"], false);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn refuses_foreign_or_invalid_host_without_touching_it() {
        let dir = sandbox();
        let binary = fake_binary(&dir);
        let foreign = json!({
            "name":HOST_NAME,"type":"stdio","description":"Other app",
            "path":binary.to_string_lossy(),
            "allowed_origins":[format!("chrome-extension://{A}/")]
        });
        let path = manifest_path(&dir);
        fs::write(&path, serde_json::to_vec(&foreign).unwrap()).unwrap();
        assert!(install_at(&dir, &binary, A, false).is_err());
        assert!(uninstall_at(&dir).is_err());
        assert!(rollback_at(&dir).is_err());
        assert_eq!(read_manifest(&path).unwrap().unwrap(), foreign);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn refuses_symlinked_manifest_and_invalid_extensions() {
        assert!(!valid_extension_id("malformed"));
        assert!(!valid_extension_id("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"));
        let dir = sandbox();
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let elsewhere = dir.join("elsewhere");
            fs::write(&elsewhere, b"other").unwrap();
            symlink(&elsewhere, manifest_path(&dir)).unwrap();
            assert!(status_at(&dir).is_err());
            assert!(uninstall_at(&dir).is_err());
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
