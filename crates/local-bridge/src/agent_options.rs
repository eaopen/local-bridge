//! One Web V2 Agent status, selection, run and read contract for WorkBuddy,
//! Codex and Pi. Unknown login/API readiness is NEVER reported as verified.
//! Selection is before execution only; failed/unknown tasks never switch Agent.
use crate::{agent_grants, agent_preferences, agent_task, inbox, workbuddy_web};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};

fn default_auto() -> String {
    "auto".into()
}
fn default_prefer() -> String {
    "prefer".into()
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAgentRequest {
    pub api_version: u32,
    pub client_key: String,
    pub prompt: String,
    #[serde(default = "default_auto")]
    pub agent: String,
    #[serde(default = "default_auto")]
    pub model: String,
    #[serde(default = "default_auto")]
    pub think_mode: String,
    #[serde(default = "default_prefer")]
    pub agent_mode: String,
}
fn valid_id(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':' | b'/'))
}
impl RunAgentRequest {
    pub fn check(&self) -> Result<()> {
        if self.api_version != 2
            || !valid_id(&self.client_key, 80)
            || self.client_key.contains('/')
            || self.client_key.contains(':')
            || self.prompt.trim().is_empty()
            || self.prompt.len() > 12_000
            || self.prompt.contains('\0')
            || !["auto", "workbuddy", "codex", "pi"].contains(&self.agent.as_str())
            || !valid_id(&self.model, 128)
            || self.model.contains("://")
            || self.model.starts_with('/')
            || self.model.contains("..")
            || !["auto", "off", "low", "medium", "high", "xhigh", "max"]
                .contains(&self.think_mode.as_str())
            || !["prefer", "require"].contains(&self.agent_mode.as_str())
            || (self.agent == "auto" && self.agent_mode == "require")
        {
            bail!("invalid agent run options");
        }
        Ok(())
    }
}
fn installed_cli(name: &str) -> bool {
    let binary = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let paths = std::env::var_os("PATH").unwrap_or_default();
    #[cfg(target_os = "macos")]
    let dirs: Vec<_> = std::env::split_paths(&paths)
        .chain(
            ["/opt/homebrew/bin", "/usr/local/bin"]
                .into_iter()
                .map(std::path::PathBuf::from),
        )
        .collect();
    #[cfg(not(target_os = "macos"))]
    let dirs: Vec<_> = std::env::split_paths(&paths).collect();
    dirs.iter().any(|dir| {
        let path = dir.join(&binary);
        if !path.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            path.metadata()
                .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}
pub(crate) fn options_supported(agent: &str, model: &str, think: &str) -> bool {
    match agent {
        "workbuddy" => model == "auto" && think == "auto",
        "codex" => {
            (model == "auto" || valid_id(model, 128))
                && ["auto", "low", "medium", "high", "xhigh"].contains(&think)
        }
        "pi" => {
            (model == "auto" || valid_id(model, 128))
                && ["auto", "off", "low", "medium", "high", "xhigh"].contains(&think)
        }
        _ => false,
    }
}
fn agent_state(origin: &str, name: &str) -> Result<Value> {
    let authorized = agent_grants::allowed(origin, name)?;
    let (installed, run_supported, modes) = match name {
        "workbuddy" => {
            let wb = workbuddy_web::capability(origin)?;
            (
                wb["installed"] == true,
                wb["run_supported"] == true,
                vec!["auto"],
            )
        }
        "codex" => (
            installed_cli("codex"),
            true,
            vec!["auto", "low", "medium", "high", "xhigh"],
        ),
        "pi" => (
            installed_cli("pi"),
            true,
            vec!["auto", "off", "low", "medium", "high", "xhigh"],
        ),
        _ => bail!("unknown Agent provider"),
    };
    let eligible = installed && authorized && run_supported;
    // An installed CLI or WBIPC application is NOT proof of usable login,
    // a valid model, quota or successful completion. Real work resolves that.
    let state = if !installed {
        "not_installed"
    } else if !run_supported {
        "unsupported_platform"
    } else if !authorized {
        "not_granted"
    } else {
        "unverified"
    };
    Ok(json!({
        "agent":name,
        "installed":installed,
        "local_grant":authorized,
        "run_supported":run_supported,
        "eligible":eligible,
        "state":state,
        "authenticated":null,
        "api_ready":null,
        "ready":null,
        "task_mode":if name == "codex" { "read_only_sandbox" } else { "text_only_no_tools" },
        "supported_think_modes":modes,
        "supports_model_override":name != "workbuddy"
    }))
}
/// Fresh, uniform status read used by native, local HTTP, CLI and the Web SDK.
/// No login probes or model calls occur during capability discovery.
pub fn capabilities(origin: &str) -> Result<Value> {
    let agents: Vec<Value> = ["workbuddy", "codex", "pi"]
        .iter()
        .map(|name| agent_state(origin, name))
        .collect::<Result<_>>()?;
    Ok(json!(agents))
}
fn pick_agent<'a>(requested: &str, mode: &str, candidates: &'a [&'a str]) -> Result<&'a str> {
    if mode == "require" {
        return candidates
            .iter()
            .copied()
            .find(|c| *c == requested)
            .ok_or_else(|| anyhow::anyhow!("required Agent unavailable or incompatible"));
    }
    if requested != "auto" {
        if let Some(selected) = candidates.iter().copied().find(|c| *c == requested) {
            return Ok(selected);
        }
    }
    candidates
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("no authorized compatible local Agent"))
}

/// Serialize task starts across both native Host and HTTP server processes.
/// Never hold the lock while a running Agent or its worker performs a task.
// Keep one OS file lock across process boundaries while resolving a clientKey
// and recording its task. Unlike create_new + delete, this releases on crash
// and cannot delete another process's live lock after an arbitrary 60s cutoff.
struct StartLock {
    _file: fs::File,
}
impl StartLock {
    fn acquire() -> Result<Self> {
        let dir = inbox::data_root()?;
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        Self::acquire_at(&dir.join("agent-v2-start.lock"))
    }

    fn acquire_at(path: &std::path::Path) -> Result<Self> {
        if fs::symlink_metadata(path).is_ok_and(|m| !m.file_type().is_file()) {
            bail!("invalid Agent start lock");
        }
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(path).context("cannot open Agent start lock")?;
        match file.try_lock().map_err(std::io::Error::from) {
            Ok(()) => Ok(Self { _file: file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                bail!("Agent task submission already in progress; preserve clientKey")
            }
            Err(error) => Err(error).context("cannot lock Agent task submission"),
        }
    }
}
fn accepted(id: &str, chosen: &str, request: &RunAgentRequest, reused: bool) -> Value {
    json!({
        "api_version":2,
        "task_id":id,"status":"accepted","reused":reused,
        "selected_agent":chosen,"requested_agent":request.agent,
        "selection":if reused { "existing_task" } else if request.agent == chosen {
            "preferred"
        } else if request.agent == "auto" {
            "automatic"
        } else {
            "fallback_before_execution"
        },
        "requested_model":request.model,"requested_think_mode":request.think_mode,
        "applied_model":null,"applied_think_mode":null
    })
}
pub fn run(origin: &str, request: RunAgentRequest) -> Result<Value> {
    request.check()?;
    let _lock = StartLock::acquire()?;
    // Resolve an existing client key BEFORE selecting another Agent. In a
    // transport failure the same logical task must not be started twice.
    if let Some(id) = workbuddy_web::find(origin, &request.client_key)? {
        if !agent_grants::allowed(origin, "workbuddy")? {
            bail!("existing WorkBuddy task grant revoked");
        }
        return Ok(accepted(&id, "workbuddy", &request, true));
    }
    if let Some((id, provider)) = agent_task::find(origin, &request.client_key)? {
        if !agent_grants::allowed(origin, &provider)? {
            bail!("existing Agent task grant revoked");
        }
        return Ok(accepted(&id, &provider, &request, true));
    }
    // Persisted user priority only changes candidate order. The exact
    // per-origin grant and selected Agent capabilities still apply.
    let prefs = agent_preferences::load()?;
    let candidates: Vec<&str> = prefs
        .priority
        .iter()
        .map(String::as_str)
        .filter(|name| {
            options_supported(name, &request.model, &request.think_mode)
                && agent_state(origin, name).is_ok_and(|state| state["eligible"] == true)
        })
        .collect();
    let chosen = pick_agent(&request.agent, &request.agent_mode, &candidates)?;
    let output = match chosen {
        "workbuddy" => workbuddy_web::start(origin, &request.client_key, &request.prompt)?,
        "codex" | "pi" => agent_task::start(
            origin,
            &request.client_key,
            chosen,
            &request.prompt,
            &request.model,
            &request.think_mode,
        )?,
        _ => bail!("unsupported Agent adapter"),
    };
    let id = output["task_id"]
        .as_str()
        .context("missing Agent task ID")?;
    Ok(accepted(id, chosen, &request, output["reused"] == true))
}
pub fn poll(origin: &str, id: &str, cursor: u64) -> Result<Value> {
    let mut page = if id.starts_with("wb-") {
        workbuddy_web::poll(origin, id, cursor)?
    } else if id.starts_with("codex-") || id.starts_with("pi-") {
        agent_task::poll(origin, id, cursor)?
    } else {
        bail!("unsupported Agent task ID");
    };
    page["api_version"] = json!(2);
    Ok(page)
}
pub fn from_value(value: &Value) -> Result<RunAgentRequest> {
    let values = agent_preferences::apply_defaults(value, &agent_preferences::load()?)?;
    let parsed: RunAgentRequest = serde_json::from_value(values)?;
    parsed.check()?;
    Ok(parsed)
}

/// Read-only OS-local installed/capability hints for the optional desktop.
/// No origin grant, authenticated=true or API readiness is implied.
pub fn local_inventory() -> Value {
    json!([
        {
            "agent":"workbuddy",
            "installed":workbuddy_web::installed(),
            "run_supported":cfg!(target_os="macos"),
            "authenticated":null,
            "api_ready":null,
            "supported_think_modes":["auto"],
            "supports_model_override":false,
        },
        {
            "agent":"codex",
            "installed":installed_cli("codex"),
            "run_supported":true,
            "authenticated":null,
            "api_ready":null,
            "supported_think_modes":["auto","low","medium","high","xhigh"],
            "supports_model_override":true,
        },
        {
            "agent":"pi",
            "installed":installed_cli("pi"),
            "run_supported":true,
            "authenticated":null,
            "api_ready":null,
            "supported_think_modes":["auto","off","low","medium","high","xhigh"],
            "supports_model_override":true,
        }
    ])
}
/// Native desktop selection shares Web V2 preferences, option validation and
/// selection rules, but never manufactures a Web origin grant. This is not
/// exposed by HTTP, Native Messaging or MCP.
pub fn local_selection() -> Result<Value> {
    let prefs = agent_preferences::load()?;
    let inventory = local_inventory();
    let candidates: Vec<&str> = prefs
        .priority
        .iter()
        .map(String::as_str)
        .filter(|name| {
            options_supported(name, &prefs.model, &prefs.think_mode)
                && inventory.as_array().is_some_and(|items| {
                    items.iter().any(|item| {
                        item["agent"] == *name
                            && item["installed"] == true
                            && item["run_supported"] == true
                    })
                })
        })
        .collect();
    let chosen = pick_agent(&prefs.agent, &prefs.agent_mode, &candidates)?;
    Ok(json!({"agent":chosen,"model":prefs.model,"think_mode":prefs.think_mode}))
}

/// Terminal, non-persisting mode of the same Agent adapter dispatch. Selection
/// is pinned to what the native UI displayed; there is no post-start fallback.
pub(crate) fn run_local(request: RunAgentRequest) -> Result<Value> {
    request.check()?;
    if request.agent_mode != "require"
        || !options_supported(&request.agent, &request.model, &request.think_mode)
    {
        bail!("private text run requires one explicit compatible Agent");
    }
    let workspace = crate::agent_health::Workspace::new()?;
    match request.agent.as_str() {
        "codex" => crate::agent::run_codex_text(
            &request.prompt,
            &request.model,
            &request.think_mode,
            &workspace.0,
        ),
        "pi" => crate::agent::run_pi_text(
            &request.prompt,
            &request.model,
            &request.think_mode,
            &workspace.0,
        ),
        "workbuddy" => workbuddy_web::run_text(&request.prompt, &workspace.0),
        _ => bail!("unsupported Agent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_start_lock_excludes_concurrent_starts_and_recovers_without_deleting_file() {
        let path = std::env::temp_dir().join(format!(
            "bridge-agent-lock-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_file(&path);
        let first = StartLock::acquire_at(&path).unwrap();
        assert!(StartLock::acquire_at(&path).is_err());
        drop(first);
        assert!(path.is_file(), "stable OS lock file must not be unlinked");
        let second = StartLock::acquire_at(&path).unwrap();
        drop(second);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn selection_and_strictness_are_shared_by_all_adapters() {
        assert_eq!(
            pick_agent("auto", "prefer", &["codex", "pi"]).unwrap(),
            "codex"
        );
        assert_eq!(pick_agent("pi", "require", &["codex", "pi"]).unwrap(), "pi");
        assert_eq!(
            pick_agent("workbuddy", "prefer", &["codex", "pi"]).unwrap(),
            "codex"
        );
        assert!(pick_agent("workbuddy", "require", &["codex", "pi"]).is_err());
        assert!(pick_agent("auto", "prefer", &[]).is_err());
    }
    #[test]
    fn provider_options_must_be_supported_before_submitting_any_task() {
        assert!(options_supported("workbuddy", "auto", "auto"));
        assert!(!options_supported("workbuddy", "gpt-6", "auto"));
        assert!(options_supported("codex", "gpt-6", "high"));
        assert!(!options_supported("codex", "auto", "off"));
        assert!(options_supported("pi", "9router/m3cn/model", "medium"));
        assert!(!options_supported("pi", "auto", "max"));
    }
    #[test]
    fn invalid_options_fail_before_start() {
        let base = json!({"api_version":2,"client_key":"task-123","prompt":"synthetic task"});
        assert!(from_value(&base).is_ok());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"synthetic"})).is_err());
        assert!(
            from_value(&json!({"api_version":1,"client_key":"task-123","prompt":"synthetic"}))
                .is_err()
        );
        assert!(from_value(
            &json!({"api_version":2,"client_key":"task-123","prompt":"test","shell":"rm"})
        )
        .is_err());
        assert!(from_value(&json!({"api_version":2,"client_key":"task-123","prompt":"test","agent":"auto","agent_mode":"require"})).is_err());
        assert!(from_value(
            &json!({"api_version":2,"client_key":"task-123","prompt":"test","model":"../secret"})
        )
        .is_err());
    }
}
