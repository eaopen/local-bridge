//! Explicit experimental macOS acceptance harness, not a shipped AgentAdapter.
//! The external helper drives ACP; this Rust process independently reads its output file.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let receipt_path = if args.len() == 2 && args[0] == "--verify" {
        PathBuf::from(&args[1])
    } else if args.is_empty() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir =
            std::env::temp_dir().join(format!("bridge-wb-receipt-{}-{stamp}", std::process::id()));
        fs::create_dir(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let receipt = dir.join("receipt.json");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/workbuddy-native-validation/run.mjs");
        let status = Command::new("node")
            .args(["--experimental-transform-types"])
            .arg(script)
            .env("LOCAL_BRIDGE_VALIDATION_RECEIPT", &receipt)
            .status()
            .context("Node 24+ is required for this optional validation helper")?;
        if !status.success() {
            bail!("WorkBuddy validation helper failed; do not replay automatically");
        }
        receipt
    } else {
        bail!("usage: workbuddy_native_validation [--verify <receipt.json>]")
    };
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path)?)?;
    let root = PathBuf::from(receipt["root"].as_str().context("missing root")?);
    let target = root.join("workspace/bridge-workbuddy-test.txt");
    if receipt["result_path"].as_str() != target.to_str() {
        bail!("result path mismatch");
    }
    if fs::symlink_metadata(&target)?.file_type().is_symlink() {
        bail!("unexpected symlink");
    }
    let events = receipt["events"].as_array().context("missing ACP events")?;
    if !events.iter().any(|e| {
        e["event"] == "prompt_completed"
            && e["stopReason"] == "end_turn"
            && e["text"]
                .as_str()
                .is_some_and(|s| s.contains(target.to_str().unwrap_or("")))
    }) {
        bail!("no matching ACP final result");
    }
    if !events
        .iter()
        .any(|e| e["event"] == "tool_event" && e["status"] == "completed")
    {
        bail!("no completed tool event");
    }
    let bytes = fs::read(&target)?;
    if bytes != b"LOCAL_BRIDGE_WORKBUDDY_OK" {
        bail!("independent Rust file verification failed");
    }
    println!(
        "{}",
        json!({"status":"completed", "validation_only":true,
        "verifier":"local-bridge Rust example", "session_id":receipt["session_id"],
        "path":target, "bytes":bytes.len(), "content_verified":true, "receipt":receipt_path})
    );
    Ok(())
}
