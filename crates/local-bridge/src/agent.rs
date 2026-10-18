//! User-invoked Personal Agent launcher. Never an MCP or enterprise Channel tool.
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CATALOG_TIMEOUT: Duration = Duration::from_secs(15);
const PREFERRED_MODELS: &[&str] = &[
    "opencode/mimo-v2.6-flash-free",
    "opencode/mimo-v2.5-free",
    "opencode/nemotron-3.5-lightning-free",
];

fn usage() {
    println!(
        "Local Bridge Personal Agent (local, opt-in)\n\
         Commands:\n\
           agent                  OpenCode free-model terminal chat\n\
           agent doctor           Check Pi / OpenCode installation\n\
           agent models           Show OpenCode free-model candidates\n\
           agent opencode [--model opencode/name-free] [--dir <path>]\n\
           agent pi [--model provider/model] [--dir <path>]\n\
         The default workspace is separate from your working directory.\n\
         Free quotas may change. No automatic download, paid fallback or remote execution."
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
        "Install official clients as your normal user:\n\
         OpenCode https://opencode.ai/docs/\n\
         Pi       https://pi.dev/\n\
         Restart your terminal and run: local-bridge agent doctor\n\
         Provider credentials stay with the official CLIs."
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

fn choose_default(models: &[String]) -> Option<&str> {
    for preferred in PREFERRED_MODELS {
        if models.iter().any(|value| value == preferred) {
            return Some(preferred);
        }
    }
    models.first().map(String::as_str)
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
    let available = free_models()?;
    let model = match &selected.model {
        Some(model) if !is_free_model(model) => {
            bail!("starter only allows official OpenCode model IDs ending in -free")
        }
        Some(model) if !available.contains(model) => {
            bail!("model not in the current free catalog; run local-bridge agent models")
        }
        Some(model) => model.as_str(),
        None => choose_default(&available).context(
            "no -free models in current OpenCode catalog; try opencode models --refresh",
        )?,
    };
    let directory = workspace(selected.directory.as_deref())?;
    eprintln!(
        "Local Bridge > OpenCode: {model}\nWorkspace: {}\n\
         Messages may leave your device; provider quotas can change.\n\
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

fn pi_chat(selected: &Options) -> Result<()> {
    let directory = workspace(selected.directory.as_deref())?;
    eprintln!(
        "Local Bridge > Pi (chat-only, no tools)\nWorkspace: {}\n\
         Use Pi /login for your own account. Model choice may be billable.",
        directory.display()
    );
    let mut command = Command::new("pi");
    command.arg("--no-tools");
    if let Some(model) = selected.model.as_deref() {
        command.args(["--model", model]);
    }
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
    let opencode = installed("opencode");
    let pi = installed("pi");
    println!("Local Bridge Personal Agent");
    println!("OpenCode installed: {opencode}");
    println!("Pi installed:       {pi}");
    println!("Start: local-bridge agent");
    if !opencode || !pi {
        install_guide();
    }
}

pub fn run(args: &[String]) -> Result<()> {
    match args.split_first() {
        None => opencode_chat(&Options::empty()),
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
        Some((command, rest)) if command == "opencode" => opencode_chat(&options(rest)?),
        Some((command, rest)) if command == "pi" => pi_chat(&options(rest)?),
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
        assert_eq!(
            choose_default(&models),
            Some("opencode/mimo-v2.6-flash-free")
        );
        assert!(!is_free_model("opencode/mimo-v2.6-flash"));
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
