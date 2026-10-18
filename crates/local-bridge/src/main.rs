mod agent;
mod cloudfile;
mod mcp;

use anyhow::{bail, Context, Result};
use local_bridge_core::{
    validate_manifest, visible_channels, ChannelManifest, AVAILABLE_CAPABILITIES,
};
use serde_json::json;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [x] if x == "--version" || x == "version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        [x] if x == "status" => {
            println!(
                "{}",
                json!({
                    "name":"local-bridge","version":env!("CARGO_PKG_VERSION"),
                    "transport":"mcp-stdio","cloudfile_configured":cloudfile::configured(),
                    "capabilities":AVAILABLE_CAPABILITIES
                })
            );
        }
        [x] if x == "mcp" => mcp::serve()?,
        [command, rest @ ..] if command == "agent" => agent::run(rest)?,
        [command, filename] if command == "validate-channel" => {
            // File validation does not grant trust to a remote origin.
            let path = Path::new(filename);
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() || metadata.len() > 64 * 1024 {
                bail!("invalid enterprise channel file size");
            }
            let value: ChannelManifest = serde_json::from_slice(&std::fs::read(path)?)?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("invalid system clock")?
                .as_secs();
            validate_manifest(&value, now)?;
            println!(
                "{}",
                json!({"valid":true, "provider":value.provider,
                "visible_channels":visible_channels(&value, AVAILABLE_CAPABILITIES)})
            );
        }
        [] | [_] if args.first().is_none_or(|s| s == "--help" || s == "-h") => {
            println!(
                "Local Bridge {}\nCommands: status | mcp | agent [chat|code|doctor|models] | validate-channel <file> | --version",
                env!("CARGO_PKG_VERSION")
            );
        }
        _ => bail!("unknown Local Bridge command"),
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Local Bridge: {e:#}");
        std::process::exit(1);
    }
}
