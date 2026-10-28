mod cloudfile;
mod mcp;

use anyhow::{bail, Context, Result};
use local_bridge::{agent, inbox, mcp_provider, native, workbuddy};
use local_bridge_core::profile::{validate_profile_shape, ApplicationProfile};
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
        [x, action, id, executable, fixed_args @ ..] if x == "provider" && action == "add" => {
            println!("{}", mcp_provider::add(id, executable, fixed_args)?);
        }
        [x, action] if x == "provider" && action == "list" => {
            println!("{}", mcp_provider::list()?);
        }
        [x, action, id] if x == "provider" && action == "remove" => {
            println!("{}", mcp_provider::remove(id)?);
        }
        [x, action, id] if x == "provider" && action == "tools" => {
            println!("{}", mcp_provider::tools(id)?);
        }
        [x, action, id, tool, json_input] if x == "provider" && action == "call" => {
            println!("{}", mcp_provider::call(id, tool, json_input)?);
        }
        [x, action, id] if x == "native-host" && action == "install" => {
            println!("{}", native::install_host(id, false)?);
        }
        [x, action, id, flag]
            if x == "native-host" && action == "install" && flag == "--replace" =>
        {
            println!("{}", native::install_host(id, true)?);
        }
        [x, action, id] if x == "relay" && action == "prepare" => {
            println!("{}", inbox::relay_prepare(id)?);
        }
        [x, action] if x == "inbox" && action == "list" => {
            println!("{}", inbox::list()?);
        }
        [x, action, id] if x == "inbox" && action == "read" => {
            println!("{}", inbox::read(id)?);
        }
        [x, action, id] if x == "inbox" && action == "delete" => {
            println!("{}", inbox::delete(id)?);
        }
        [x, action, provider, id]
            if x == "inbox" && action == "summarize" && provider == "codex" =>
        {
            // User chooses a stored item AND explicitly invokes their existing
            // authenticated Codex CLI. Browser share alone never starts AI.
            let prompt = inbox::summarize_prompt(id)?;
            let result = agent::run_codex(&prompt)?;
            println!(
                "{}",
                json!({
                    "inbox_id":id,
                    "provider":"codex",
                    "status":result["status"],
                    "result":result["result"],
                    "persisted_by_bridge":false
                })
            );
        }
        [command, provider, action]
            if command == "agent" && provider == "workbuddy" && action == "status" =>
        {
            println!("{}", workbuddy::Client::from_env()?.status()?);
        }
        [command, provider, action, value] if command == "agent" && provider == "workbuddy" => {
            let client = workbuddy::Client::from_env()?;
            let output = match action.as_str() {
                "send" => {
                    let prompt = if value == "-" {
                        agent::prompt_from_stdin()?
                    } else {
                        value.clone()
                    };
                    client.send(&prompt)?
                }
                "messages" => client.messages(value)?,
                _ => bail!("expected workbuddy status | send <prompt|-> | messages <message-id>"),
            };
            println!("{}", output);
        }
        [agent, provider, prompt]
            if agent == "agent" && (provider == "codex" || provider == "codebuddy") =>
        {
            let input = if prompt == "-" {
                agent::prompt_from_stdin()?
            } else {
                prompt.clone()
            };
            let output = if provider == "codex" {
                agent::run_codex(&input)?
            } else {
                agent::run_codebuddy(&input)?
            };
            println!("{}", output);
        }
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
        [command, filename] if command == "validate-profile" => {
            // Shape-only: a local file is not proof of trusted origin or authorization.
            let path = Path::new(filename);
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() || metadata.len() > 64 * 1024 {
                bail!("invalid application profile file size");
            }
            let value: ApplicationProfile = serde_json::from_slice(&std::fs::read(path)?)?;
            validate_profile_shape(&value)?;
            println!(
                "{}",
                json!({"shape_valid":true, "trusted":false, "activated":false,
                    "profile_id":value.profile_id,
                    "channels":value.channels.len()})
            );
        }
        [] | [_] if args.first().is_none_or(|s| s == "--help" || s == "-h") => {
            println!(
                "Local Bridge {}\nCommands: status | mcp | provider add <id> <absolute-executable> [fixed-args...] | provider list | provider tools <id> | provider call <id> <tool> <json|-> | provider remove <id> | relay prepare <inbox-id> | agent codex|codebuddy <prompt|-> | agent workbuddy status|send <prompt|->|messages <message-id> | inbox list|read <id>|delete <id>|summarize codex <id> | native-host install <extension-id> [--replace] | validate-channel <file> | validate-profile <file> | --version",
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
