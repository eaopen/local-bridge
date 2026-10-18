//! User-invoked Personal Agent launcher. Never an MCP or enterprise Channel tool.
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CATALOG_TIMEOUT: Duration = Duration::from_secs(15);
// Preset one reasonable free-tier model per mode; never select a random candidate.
const DEFAULT_PI_PROVIDER: &str = "google";
const DEFAULT_PI_MODEL: &str = "gemini-3.5-flash-lite";
const DEFAULT_CODE_MODEL: &str = "opencode/deepseek-v4.1-flash-free";

fn usage() {
    println!(
        "Local Bridge Personal Agent\n\
         Commands:\n\
           agent                  Chat with Pi (preset Gemini free-tier model)\n\
           agent chat             Same as agent\n\
           agent code             Code with OpenCode (preset free candidate)\n\
           agent doctor           Check required CLIs\n\
           agent models           Diagnose OpenCode free candidates\n\
           agent [chat|code] --dir <existing-folder>\n\
         Advanced: agent pi|opencode [--model provider/model] [--dir <folder>]\n\
         The user supplies provider credentials once. No automatic paid fallback."
    );
}

fn installed(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|result| result.success())
}

fn install_guide() {
    eprintln!(
        "Install Pi as your normal user: https://pi.dev/docs/latest/quickstart\n\
         Optional coding mode: https://opencode.ai/docs/\n\
         For Pi's default free tier, use a Google AI Studio API key on a project\n\
         without paid billing: https://aistudio.google.com/api-keys\n\
         In Pi, run /login google once. No model selection required."
    );
}

fn is_free_model(model: &str) -> bool {
    let Some(name) = model.strip_prefix("opencode/") else {
        return false;
    };
    !name.is_empty()
        && model.len() <= 128
        && name.ends_with("-free")
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_' | b'.')
        })
}

fn parse_free_models(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|model| is_free_model(model))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Model IDs are supplied by the official installed OpenCode CLI, not a
/// third-party proxy. A bounded local subprocess is never exposed through MCP.
fn free_models() -> Result<Vec<String>> {
    let mut child = Command::new("opencode")
        .args(["models", "opencode"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("OpenCode CLI not found; run local-bridge agent doctor")?;
    let deadline = Instant::now() + CATALOG_TIMEOUT;
    loop {
        match child
            .try_wait()
            .context("cannot poll OpenCode model catalog")?
        {
            Some(status) if status.success() => break,
            Some(status) => bail!("OpenCode model catalog failed: {status}"),
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("OpenCode model catalog timed out; try opencode models opencode directly");
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let output = child
        .wait_with_output()
        .context("cannot read OpenCode model list")?;
    if output.stdout.len() > 256 * 1024 {
        bail!("OpenCode model catalog unexpectedly large");
    }
    Ok(parse_free_models(&String::from_utf8_lossy(&output.stdout)))
}

fn workspace(explicit: Option<&str>) -> Result<PathBuf> {
    if let Some(directory) = explicit {
        let path = Path::new(directory);
        if !path.is_dir() {
            bail!("--dir must name an existing directory");
        }
        return path.canonicalize().context("invalid --dir directory");
    }
    #[cfg(windows)]
    let home = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("USERPROFILE"));
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    let home = home.context("cannot find the current user's home directory")?;
    let directory = PathBuf::from(home)
        .join(".local-bridge")
        .join("agent-workspace");
    std::fs::create_dir_all(&directory).context("cannot create Personal Agent workspace")?;
    Ok(directory)
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    model: Option<String>,
    directory: Option<String>,
}

impl Options {
    fn empty() -> Self {
        Self {
            model: None,
            directory: None,
        }
    }
}

fn options(args: &[String]) -> Result<Options> {
    let mut result = Options::empty();
    let mut index = 0;
    while index < args.len() {
        let value = args.get(index + 1);
        match args[index].as_str() {
            "--model" if result.model.is_none() => {
                let model = value.context("--model needs a provider/model ID")?;
                if model.len() > 128
                    || !model.contains('/')
                    || !model.bytes().all(|b| {
                        b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.')
                    })
                {
                    bail!("invalid model ID");
                }
                result.model = Some(model.clone());
            }
            "--dir" if result.directory.is_none() => {
                let directory = value.context("--dir needs a path")?;
                if directory.is_empty() || directory.len() > 4096 {
                    bail!("invalid --dir path");
                }
                result.directory = Some(directory.clone());
            }
            _ => bail!("unknown or repeated Personal Agent option: {}", args[index]),
        }
        index += 2;
    }
    Ok(result)
}

fn safe_config_override(mut config: Value) -> Result<Value> {
    let root = config
        .as_object_mut()
        .context("OpenCode inline configuration must be a JSON object")?;
    let previous = root.remove("permission").unwrap_or_else(|| json!({}));
    let mut permissions = match previous {
        Value::Object(map) => map,
        Value::String(action) if matches!(action.as_str(), "allow" | "ask" | "deny") => {
            let mut map = Map::new();
            map.insert("*".into(), Value::String(action));
            map
        }
        _ => bail!("invalid OpenCode permission configuration"),
    };
    // OpenCode otherwise allows shell and writes by default. Never auto-approve.
    for name in ["bash", "edit", "external_directory"] {
        permissions.insert(name.into(), json!("ask"));
    }
    root.insert("permission".into(), Value::Object(permissions));
    Ok(config)
}

fn safe_opencode_config() -> Result<String> {
    let config: Value = match std::env::var("OPENCODE_CONFIG_CONTENT") {
        Ok(content) if !content.is_empty() => {
            serde_json::from_str(&content).context("invalid OPENCODE_CONFIG_CONTENT")?
        }
        _ => json!({}),
    };
    serde_json::to_string(&safe_config_override(config)?)
        .context("cannot encode OpenCode permissions")
}

fn opencode_chat(selected: &Options) -> Result<()> {
    let model = selected.model.as_deref().unwrap_or(DEFAULT_CODE_MODEL);
    if !is_free_model(model) {
        bail!("coding starter only accepts official OpenCode -free model IDs");
    }
    let available = free_models()?;
    if !available.iter().any(|listed| listed == model) {
        bail!(
            "preset coding model {model} is not in the current OpenCode free catalog. \
             No paid or random fallback. Use local-bridge agent models for diagnosis."
        );
    }
    let directory = workspace(selected.directory.as_deref())?;
    eprintln!(
        "Local Bridge > OpenCode coding: {model}\nWorkspace: {}\n\
         Free quotas and eligibility can change.\n\
         Shell/file writes require local confirmation. No MCP remote execution.",
        directory.display()
    );
    let status = Command::new("opencode")
        .args(["--model", model, "--agent", "plan"])
        .current_dir(directory)
        .env("OPENCODE_CONFIG_CONTENT", safe_opencode_config()?)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("cannot start official OpenCode CLI")?;
    if !status.success() {
        bail!("OpenCode exited with {status}");
    }
    Ok(())
}

fn pi_args(custom_model: Option<&str>) -> Vec<&str> {
    match custom_model {
        Some(model) => vec!["--no-tools", "--model", model],
        None => vec![
            "--no-tools",
            "--provider",
            DEFAULT_PI_PROVIDER,
            "--model",
            DEFAULT_PI_MODEL,
        ],
    }
}

fn pi_chat(selected: &Options) -> Result<()> {
    let directory = workspace(selected.directory.as_deref())?;
    let custom_model = selected.model.as_deref();
    let shown_model = custom_model.unwrap_or("google/gemini-3.5-flash-lite");
    eprintln!(
        "Local Bridge > Pi personal chat (no local tools)\n\
         Model: {shown_model}\nWorkspace: {}\n\
         On first use, enter your Google AI Studio free-tier key in Pi via /login google.\n\
         Free tier depends on regional eligibility, quotas and project billing settings.",
        directory.display()
    );
    if custom_model.is_some() {
        eprintln!("Custom Pi model was explicitly requested; check its pricing.");
    }
    let mut command = Command::new("pi");
    command.args(pi_args(custom_model));
    let status = command
        .current_dir(directory)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Pi CLI not found; run local-bridge agent doctor")?;
    if !status.success() {
        bail!("Pi exited with {status}");
    }
    Ok(())
}

fn doctor() {
    let pi = installed("pi");
    let opencode = installed("opencode");
    println!("Local Bridge Personal Agent");
    println!("Pi installed (chat): {pi}");
    println!("OpenCode installed (optional coding): {opencode}");
    println!("Default chat: {DEFAULT_PI_PROVIDER}/{DEFAULT_PI_MODEL}");
    println!("Default coding: {DEFAULT_CODE_MODEL}");
    println!("No model picker. A free-tier Google API key is needed once for Pi.");
    if !pi {
        install_guide();
    } else if !opencode {
        eprintln!("For optional coding mode: https://opencode.ai/docs/");
    }
}

pub fn run(args: &[String]) -> Result<()> {
    match args.split_first() {
        None => pi_chat(&Options::empty()),
        Some((command, _)) if matches!(command.as_str(), "-h" | "--help" | "help") => {
            usage();
            Ok(())
        }
        Some((command, rest)) if command == "doctor" && rest.is_empty() => {
            doctor();
            Ok(())
        }
        Some((command, rest)) if command == "models" && rest.is_empty() => {
            let available = free_models()?;
            println!("Official OpenCode -free model candidates (not an uptime guarantee):");
            for model in &available {
                println!("  {model}");
            }
            if available.is_empty() {
                eprintln!("No free models found; try opencode models --refresh");
            }
            Ok(())
        }
        Some((command, rest)) if matches!(command.as_str(), "code" | "opencode") => {
            opencode_chat(&options(rest)?)
        }
        Some((command, rest)) if matches!(command.as_str(), "chat" | "pi") => {
            pi_chat(&options(rest)?)
        }
        _ => {
            usage();
            bail!("unknown Personal Agent command")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_catalog_filters_paid_and_unsafe_ids() {
        let models = parse_free_models(
            "opencode/nemotron-3.5-lightning-free\n\
             openrouter/model-free\nopencode/big-pickle\n\
             opencode/mimo-v2.6-flash-free\nopencode/mimo-v2.6-flash-free\n\
             opencode/a-free --auto\n",
        );
        assert_eq!(models.len(), 2);
        assert_eq!(DEFAULT_CODE_MODEL, "opencode/deepseek-v4.1-flash-free");
        assert_eq!(DEFAULT_PI_PROVIDER, "google");
        assert_eq!(DEFAULT_PI_MODEL, "gemini-3.5-flash-lite");
        assert!(!is_free_model("opencode/mimo-v2.6-flash"));
    }

    #[test]
    fn pi_defaults_are_fixed_and_do_not_enable_tools() {
        assert_eq!(
            pi_args(None),
            vec![
                "--no-tools",
                "--provider",
                "google",
                "--model",
                "gemini-3.5-flash-lite"
            ]
        );
        assert_eq!(
            pi_args(Some("custom/model")),
            vec!["--no-tools", "--model", "custom/model"]
        );
        assert!(is_free_model(DEFAULT_CODE_MODEL));
    }

    #[test]
    fn malformed_options_do_not_reach_child_processes() {
        assert!(options(&["--model".into(), "--auto".into()]).is_err());
        assert!(options(&["--dir".into()]).is_err());
        assert!(options(&[
            "--model".into(),
            "opencode/a-free".into(),
            "--model".into(),
            "opencode/b-free".into()
        ])
        .is_err());
        assert_eq!(
            options(&["--model".into(), "opencode/a-free".into()])
                .unwrap()
                .model
                .as_deref(),
            Some("opencode/a-free")
        );
    }

    #[test]
    fn runtime_override_preserves_other_config_but_forces_confirmation() {
        let config = safe_config_override(json!({
            "model":"opencode/mimo-v2.6-flash-free",
            "permission":{"bash":"allow","edit":"allow","webfetch":"deny"}
        }))
        .unwrap();
        assert_eq!(config["permission"]["bash"], "ask");
        assert_eq!(config["permission"]["edit"], "ask");
        assert_eq!(config["permission"]["external_directory"], "ask");
        assert_eq!(config["permission"]["webfetch"], "deny");
        assert!(safe_config_override(json!(null)).is_err());
    }
}
