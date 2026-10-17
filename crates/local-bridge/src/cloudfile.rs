//! The first integration: communicate with the separately distributed
//! CloudFile Rust Agent2.3, never fork/copy its upload and checkout engine.
//!
//! Phase 0 only exposes the installed component's read-only identity. Actual
//! CloudFile file tools require a mutually authenticated local IPC contract
//! and remain intentionally unadvertised until that contract is tested.
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn configured() -> bool {
    executable_path().is_some()
}

fn executable_path() -> Option<std::path::PathBuf> {
    let raw = std::env::var_os("LOCAL_BRIDGE_CLOUDFILE_HOST")?;
    let path = Path::new(&raw);
    if !path.is_absolute() || !path.is_file() {
        return None;
    }
    let basename = path.file_name()?.to_string_lossy();
    if !matches!(
        basename.as_ref(),
        "cloudfile-launcher"
            | "cloudfile-launcher.exe"
            | "cloudfile-native-host"
            | "cloudfile-native-host.exe"
    ) {
        return None;
    }
    Some(path.to_path_buf())
}

pub fn sanitize_component(v: &Value) -> Value {
    json!({
        "connected": v["version"].as_str().is_some(),
        "agent_family": v["agent_family"].as_str().unwrap_or("agent2"),
        "version": v["version"].as_str().unwrap_or(""),
        "platform": v["platform"].as_str().unwrap_or(""),
        "ipc_version": v["ipc_version"].as_u64().unwrap_or(0),
        "journal_version": v["journal_version"].as_u64().unwrap_or(0)
    })
}

/// Short-lived, bounded local probe. The requesting AI never chooses a program
/// or shell command; only the LOCAL user's configured known CloudFile binary.
pub fn status() -> Value {
    let Some(program) = executable_path() else {
        return json!({"connected":false,"reason":"CloudFile Agent2 not configured"});
    };
    let child = Command::new(program)
        .arg("--component")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return json!({"connected":false,"reason":"CloudFile Agent2 cannot start"});
    };
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => {
                return json!({"connected":false,"reason":"CloudFile Agent2 probe failed"});
            }
            Ok(None) if Instant::now() >= until => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({"connected":false,"reason":"CloudFile Agent2 probe timed out"});
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
        }
    }
    // --component emits only a small version/protocol JSON value.
    let output = child.wait_with_output();
    match output.ok().and_then(|v| {
        (v.stdout.len() <= 16 * 1024)
            .then(|| serde_json::from_slice::<Value>(&v.stdout).ok())
            .flatten()
    }) {
        Some(v) if v["agent_family"] == "agent2" => sanitize_component(&v),
        _ => json!({"connected":false,"reason":"CloudFile Agent2 protocol mismatch"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_secrets_or_local_paths_leak_to_mcp() {
        let output = sanitize_component(&json!({
            "version":"2.3.0-dev.1","platform":"windows-x86_64",
            "agent_family":"agent2","ipc_version":1,"journal_version":1,
            "local_path":"C:\\secret\\project", "token":"secret"
        }));
        assert_eq!(output["connected"], true);
        assert!(output.get("token").is_none());
        assert!(output.get("local_path").is_none());
    }
}
