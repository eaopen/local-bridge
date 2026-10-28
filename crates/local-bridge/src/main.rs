mod cloudfile;
mod mcp;

use anyhow::{bail, Context, Result};
use local_bridge::{
    agent, agent_grants, agent_task, diagnostics, inbox, local_http, mcp_provider, native,
    native_host_registration, task_center, web_v2, workbuddy, workbuddy_web,
};
use local_bridge_core::profile::{validate_profile_shape, ApplicationProfile};
use local_bridge_core::profile_v2::{validate_profile_v2_shape, ApplicationProfileV2};
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
        [flag, task_id] if flag == "--internal-agent-v2-worker" => {
            agent_task::worker(task_id)?;
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
        [x] if x == "self-test" => {
            let result = diagnostics::run();
            println!("{result}");
            if result["ok"] != true {
                bail!("Local Bridge self-test reported failed checks");
            }
        }
        [x] if x == "mcp" => mcp::serve()?,
        [x, action] if x == "tasks" && action == "list" => {
            println!("{}", task_center::list(false));
        }
        [x, action, option] if x == "tasks" && action == "list" && option == "--cloudfile" => {
            println!("{}", task_center::list(true));
        }
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
        [a, action, origin, provider, tool] if a == "web-v2" && action == "grant-status" => {
            println!("{}", web_v2::grant_status(origin, provider, tool)?);
        }
        [a, action, origin, provider, tool] if a == "web-v2" && action == "revoke-status" => {
            println!("{}", web_v2::revoke_status(origin, provider, tool)?);
        }
        [a, action] if a == "web-v2" && action == "serve" => {
            local_http::serve()?;
        }
        [a, action, origin, provider] if a == "web-v2" && action == "grant-agent" => {
            println!("{}", agent_grants::grant(origin, provider)?);
        }
        [a, action, origin, provider] if a == "web-v2" && action == "revoke-agent" => {
            println!("{}", agent_grants::revoke(origin, provider)?);
        }
        [a, action, origin] if a == "web-v2" && action == "grant-workbuddy" => {
            println!("{}", workbuddy_web::grant(origin)?);
        }
        [a, action, origin] if a == "web-v2" && action == "revoke-workbuddy" => {
            println!("{}", workbuddy_web::revoke(origin)?);
        }
        [a, action, origin] if a == "web-v2" && action == "workbuddy-capability" => {
            println!("{}", workbuddy_web::capability(origin)?);
        }
        [a, action, origin] if a == "web-v2" && action == "capabilities" => {
            println!("{}", web_v2::capabilities(origin)?);
        }
        [x, action] if x == "native-host" && action == "status" => {
            println!("{}", native_host_registration::status()?);
        }
        [x, action] if x == "native-host" && action == "uninstall" => {
            println!("{}", native_host_registration::uninstall_host()?);
        }
        [x, action] if x == "native-host" && action == "rollback" => {
            println!("{}", native_host_registration::rollback_host()?);
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
        [x, agent_name, source, prompt]
            if x == "agent" && agent_name == "pi" && source == "enterprise" =>
        {
            // The trusted calling application supplies an ephemeral enterprise
            // model session. Users do not configure personal Pi model accounts.
            let input = if prompt == "-" {
                agent::prompt_from_stdin()?
            } else {
                prompt.clone()
            };
            println!("{}", agent::run_pi_enterprise(&input)?);
        }
        [x, agent_name, model_provider, model_id, prompt] if x == "agent" && agent_name == "pi" => {
            // User explicitly selects Pi's model; Pi owns the credentials.
            // Do not silently route to Codex, 9Router, or a Web AI session.
            let input = if prompt == "-" {
                agent::prompt_from_stdin()?
            } else {
                prompt.clone()
            };
            println!("{}", agent::run_pi(model_provider, model_id, &input)?);
        }
        [agent, provider, prompt] if agent == "agent" && provider == "codex" => {
            let input = if prompt == "-" {
                agent::prompt_from_stdin()?
            } else {
                prompt.clone()
            };
            let output = agent::run_codex(&input)?;
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
        [command, filename] if command == "validate-profile-v2" => {
            // Never enroll a remote enterprise Profile from an offline JSON file.
            let path = Path::new(filename);
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() || metadata.len() > 64 * 1024 {
                bail!("invalid V2 application profile file size");
            }
            let value: ApplicationProfileV2 = serde_json::from_slice(&std::fs::read(path)?)?;
            validate_profile_v2_shape(&value)?;
            println!(
                "{}",
                json!({"schema_version":2, "shape_valid":true, "trusted":false, "activated":false,
                    "profile_id":value.profile_id, "channels":value.channels.len(),
                    "mixed_mode":value.mixed_context.mode,
                    "agent_preference":value.agent_policy.as_ref().map(|p| &p.preferred_providers),
                    "enterprise_ai_enabled":value.enterprise_ai_api.as_ref().is_some_and(|a| a.enabled)})
            );
        }
        [command, action, filename] if command == "profile-v2" && action == "inspect" => {
            // User-owned local Provider registrations: never reveal executables,
            // tokens, chat data or falsely claim Provider readiness/permission.
            let path = Path::new(filename);
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() || metadata.len() > 64 * 1024 {
                bail!("invalid V2 application profile file size");
            }
            let value: ApplicationProfileV2 = serde_json::from_slice(&std::fs::read(path)?)?;
            validate_profile_v2_shape(&value)?;
            let registry = mcp_provider::list()?;
            let providers: Vec<_> = registry["providers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p["id"].as_str())
                .map(|id| json!({"id":id,"state":"configured_not_verified","authorized":false}))
                .collect();
            println!(
                "{}",
                json!({"schema_version":2,"profile_id":value.profile_id,
                    "trusted":false,"activated":false,
                    "personal_mcp_providers":providers,
                    "requested_personal_capabilities":value.mixed_context.personal_capability_requests,
                    "suggested_mcp_providers":value.mixed_context.suggested_mcp_providers,
                    "agent_preference":value.agent_policy.as_ref().map(|p| &p.preferred_providers)})
            );
        }
        [] | [_] if args.first().is_none_or(|s| s == "--help" || s == "-h") => {
            println!(
                "Local Bridge {}\nCommands: status | mcp | tasks list [--cloudfile] | provider add <id> <absolute-executable> [fixed-args...] | provider list | provider tools <id> | provider call <id> <tool> <json|-> | provider remove <id> | relay prepare <inbox-id> | agent codex <prompt|-> | agent pi enterprise <prompt|-> | agent pi <provider> <model> <prompt|-> | agent workbuddy status|send <prompt|->|messages <message-id> | inbox list|read <id>|delete <id>|summarize codex <id> | native-host status|uninstall|rollback | native-host install <extension-id> [--replace] | self-test | validate-channel <file> | validate-profile <file> | validate-profile-v2 <file> | profile-v2 inspect <file> | web-v2 grant-status <origin> <provider> <tool> | web-v2 revoke-status <origin> <provider> <tool> | web-v2 capabilities <origin> | web-v2 serve | web-v2 grant-agent <origin> <workbuddy|codex|pi> | web-v2 revoke-agent <origin> <workbuddy|codex|pi> | web-v2 grant-workbuddy <origin> | web-v2 revoke-workbuddy <origin> | --version",
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
