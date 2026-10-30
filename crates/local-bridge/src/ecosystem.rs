//! Existing-ecosystem discovery and explicit, local-only integration.
//!
//! Discovery is passive: never launches a model, probes private app data,
//! scans ports or changes a vendor's settings. Only an OS-user CLI command
//! or a Tauri button may install Pi or register Bridge with Codex MCP.
use crate::agent_options;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const PI_PACKAGE: &str = "@earendil-works/pi-coding-agent";
const LIMIT: Duration = Duration::from_secs(180);

fn executable(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH").unwrap_or_default();
    let dirs = env::split_paths(&paths);
    #[cfg(windows)]
    let names = [format!("{name}.exe"), format!("{name}.cmd")];
    #[cfg(not(windows))]
    let names = [name.to_owned(), name.to_owned()];
    #[cfg(target_os = "macos")]
    let dirs = dirs.chain(
        ["/opt/homebrew/bin", "/usr/local/bin"]
            .into_iter()
            .map(PathBuf::from),
    );
    for dir in dirs {
        for file in &names {
            let path = dir.join(file);
            if !path.is_file() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if path
                    .metadata()
                    .is_err_or(|m| m.permissions().mode() & 0o111 == 0)
                {
                    continue;
                }
            }
            return Some(path);
        }
    }
    None
}

fn home_path(relative: &str) -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(relative))
}
fn under(var: &str, relative: &str) -> Option<PathBuf> {
    env::var_os(var).map(|root| PathBuf::from(root).join(relative))
}
fn app_candidates(id: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    match (env::consts::OS, id) {
        ("macos", "utools") => {
            paths.push("/Applications/uTools.app".into());
            paths.push("/Applications/utools.app".into());
            if let Some(p) = home_path("Applications/uTools.app") {
                paths.push(p);
            }
        }
        ("macos", "raycast") => {
            paths.push("/Applications/Raycast.app".into());
            if let Some(p) = home_path("Applications/Raycast.app") {
                paths.push(p);
            }
        }
        ("macos", "shortcuts") => paths.push("/usr/bin/shortcuts".into()),
        ("windows", "utools") => {
            for item in [
                under("LOCALAPPDATA", "Programs/uTools/uTools.exe"),
                under("LOCALAPPDATA", "Programs/utools/uTools.exe"),
                under("PROGRAMFILES", "uTools/uTools.exe"),
            ]
            .into_iter()
            .flatten()
            {
                paths.push(item);
            }
        }
        ("windows", "raycast") => {
            if let Some(p) = under("LOCALAPPDATA", "Programs/Raycast/Raycast.exe") {
                paths.push(p);
            }
        }
        ("windows", "quicker") => {
            for item in [
                under("PROGRAMFILES", "Quicker/Quicker.exe"),
                under("LOCALAPPDATA", "Programs/Quicker/Quicker.exe"),
            ]
            .into_iter()
            .flatten()
            {
                paths.push(item);
            }
        }
        ("windows", "powertoys") => {
            if let Some(p) = under("PROGRAMFILES", "PowerToys/PowerToys.exe") {
                paths.push(p);
            }
        }
        ("linux", "utools") => {
            for p in [
                home_path(".local/share/applications/utools.desktop"),
                Some("/usr/share/applications/utools.desktop".into()),
                Some("/opt/uTools/utools".into()),
            ]
            .into_iter()
            .flatten()
            {
                paths.push(p);
            }
        }
        _ => {}
    }
    paths
}

fn app_found(id: &str) -> bool {
    app_candidates(id).iter().any(|p| p.exists())
}

fn entry(id: &str, kind: &str, installed: bool, next: &str, note: &str) -> Value {
    json!({"id":id,"kind":kind,"installed":installed,
        "connection_verified":false,"next":next,"note":note})
}

/// Read-only view. No use of "ready" here: installed != logged in, authorized
/// or MCP-enabled. It is valid for an app to be absent on a different OS.
pub fn discover() -> Value {
    let inventory = agent_options::local_inventory();
    let installed = |id: &str| -> bool {
        inventory.as_array().is_some_and(|rows| {
            rows.iter()
                .any(|v| v["agent"] == id && v["installed"] == true)
        })
    };
    let codex = installed("codex");
    let pi = installed("pi");
    let wb = installed("workbuddy");
    let utools = app_found("utools");
    let mut platforms = vec![
        entry("utools", "tool_host_and_optional_ai_source", utools,
            if utools { "enable_mcp_or_install_bridge_plugin" } else { "optional" },
            "uTools AI is only callable inside a permitted plugin; detection does not make it an Agent."),
        entry("raycast", "mcp_client", app_found("raycast"),
            "use_raycast_mcp_install", "Connect Bridge stdio from Raycast; no private config edits."),
    ];
    if cfg!(windows) {
        platforms.push(entry(
            "quicker",
            "automation_host",
            app_found("quicker"),
            "reuse_mcp_actions",
            "Existing Quicker actions stay managed by Quicker.",
        ));
        platforms.push(entry(
            "powertoys",
            "native_launcher",
            app_found("powertoys"),
            "optional",
            "Command Palette integration is independent of Agent choice.",
        ));
    }
    if cfg!(target_os = "macos") {
        platforms.push(entry(
            "shortcuts",
            "os_native_entry",
            app_found("shortcuts"),
            "available_without_install",
            "Native text workflows need no new launcher.",
        ));
    }
    json!({
        "schema_version":1,
        "passive":true,
        "agents":inventory,
        "platforms":platforms,
        "fallback":{
            "pi":if pi {"installed_unverified"} else if executable("node").is_some() && executable("npm").is_some() {"one_click_install_possible"} else {"requires_node_22_19"},
            "utools_ai":if utools {"plugin_adapter_required"} else {"not_detected"},
            "web_ai":"user_mediated_only",
            "reason":if codex || wb {"reuse_existing_agent_first"} else {"check_existing_auth_before_installing_pi"}
        },
        "actions":{
            "codex_connect":codex && bridge_cli().is_some(),
            "pi_install": !pi && executable("node").is_some() && executable("npm").is_some()
        }
    })
}

fn bridge_cli() -> Option<PathBuf> {
    if let Some(p) = executable("local-bridge") {
        return Some(p);
    }
    let exe = env::current_exe().ok()?;
    if exe.file_stem().is_some_and(|s| s == "local-bridge") {
        return Some(exe);
    }
    // Dev preview often places the optional Tauri window next to the CLI.
    let filename = if cfg!(windows) {
        "local-bridge.exe"
    } else {
        "local-bridge"
    };
    let sibling = exe.parent()?.join(filename);
    sibling.is_file().then_some(sibling)
}

/// Only perform the documented fixed command after a user action. Do not
/// run through a shell, rewrite vendor configuration or downgrade a package.
fn run_with_limit(mut command: Command) -> Result<()> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .context("cannot start installed setup tool")?;
    let deadline = Instant::now() + LIMIT;
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("installation / integration command failed; existing setup preserved");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("setup timed out; inspect the vendor tool before retrying");
        }
        thread::sleep(Duration::from_millis(100));
    }
}
fn parse_node_version(output: &str) -> Option<(u32, u32)> {
    let (major, rest) = output.trim().trim_start_matches('v').split_once('.')?;
    let minor = rest.split('.').next()?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}
fn node_supported(version: &str) -> bool {
    parse_node_version(version)
        .is_some_and(|(major, minor)| major > 22 || major == 22 && minor >= 19)
}

/// The only automatic package acquisition in V1. Explicit click/CLI only.
/// Pi itself owns all model auth; npm's dependency lifecycle scripts are off.
pub fn install_pi() -> Result<Value> {
    if executable("pi").is_some() {
        return Ok(json!({"id":"pi","state":"already_installed","ready":false}));
    }
    let node = executable("node").context("Node.js 22.19+ required; Pi not installed")?;
    let npm = executable("npm").context("npm unavailable; Pi not installed")?;
    let version = Command::new(node)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .context("cannot inspect Node.js version")?;
    if !version.status.success() || !node_supported(&String::from_utf8_lossy(&version.stdout)) {
        bail!("Node.js 22.19+ is required; Pi not installed");
    }
    let mut cmd = Command::new(npm);
    cmd.args(["install", "--global", "--ignore-scripts", PI_PACKAGE]);
    run_with_limit(cmd)?;
    // PATH may need refreshing after a global npm install.
    Ok(
        json!({"id":"pi","state":"install_command_completed","ready":false,
        "installed_in_current_path":executable("pi").is_some(),
        "next":"use existing Pi login/model or a trusted enterprise API; no Web Chat impersonation"}),
    )
}

fn codex_mcp_status(list: &Value, bridge_binary: &Path) -> Result<&'static str> {
    let servers = list.as_array().context("invalid Codex MCP list response")?;
    let Some(existing) = servers.iter().find(|entry| entry["name"] == "local-bridge") else {
        return Ok("not_registered");
    };
    let cmd = bridge_binary
        .to_str()
        .context("Bridge executable path is not UTF-8")?;
    if existing["transport"]["type"] == "stdio"
        && existing["transport"]["command"] == cmd
        && existing["transport"]["args"] == json!(["mcp"])
        && existing["enabled"] == true
    {
        return Ok("already_registered");
    }
    bail!("Codex already has a different local-bridge MCP entry; refusing to overwrite")
}

/// One click installs only a fixed stdio command through Codex's official CLI.
/// An existing different entry is never overwritten. Agent auth is unaffected.
pub fn connect_codex() -> Result<Value> {
    let codex = executable("codex").context("Codex CLI not installed")?;
    let bridge = bridge_cli().context("local-bridge CLI not on PATH or next to desktop app")?;
    let inspect = Command::new(&codex)
        .args(["mcp", "list", "--json"])
        .stdin(Stdio::null())
        .output()
        .context("cannot read Codex MCP list")?;
    if !inspect.status.success() || inspect.stdout.len() > 64 * 1024 {
        bail!("Codex MCP inspection failed; refusing to modify its configuration");
    }
    let list: Value =
        serde_json::from_slice(&inspect.stdout).context("unexpected Codex MCP list format")?;
    if codex_mcp_status(&list, &bridge)? == "already_registered" {
        return Ok(json!({"id":"codex","state":"already_integrated","ready":false}));
    }
    let mut cmd = Command::new(codex);
    cmd.args(["mcp", "add", "local-bridge", "--"])
        .arg(bridge)
        .arg("mcp");
    run_with_limit(cmd)?;
    Ok(json!({"id":"codex","state":"mcp_registered","ready":false,
        "next":"Codex must discover and authorize tools under its own account"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_does_not_claim_login_or_model_api() {
        let result = discover();
        assert_eq!(result["passive"], true);
        assert_eq!(result["fallback"]["web_ai"], "user_mediated_only");
        assert_eq!(result["platforms"][0]["connection_verified"], false);
    }
    #[test]
    fn node_prerequisites_are_checked_before_install() {
        assert!(node_supported("v22.19.0"));
        assert!(node_supported("v24.0.0"));
        assert!(!node_supported("v22.18.9"));
        assert!(!node_supported("v20.19.1"));
        assert!(!node_supported("garbage"));
    }
    #[test]
    fn codex_integration_is_idempotent_and_never_overwrites_other_entries() {
        let path = Path::new("/tmp/local-bridge");
        assert_eq!(
            codex_mcp_status(&json!([]), path).unwrap(),
            "not_registered"
        );
        let ok = json!([{"name":"local-bridge","enabled":true,
            "transport":{"type":"stdio","command":"/tmp/local-bridge","args":["mcp"]}}]);
        assert_eq!(codex_mcp_status(&ok, path).unwrap(), "already_registered");
        let conflict = json!([{"name":"local-bridge","enabled":true,
            "transport":{"type":"stdio","command":"/bin/other","args":["mcp"]}}]);
        assert!(codex_mcp_status(&conflict, path).is_err());
    }
}
