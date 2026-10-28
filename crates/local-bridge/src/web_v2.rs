//! Explicit, independently granted status-only MCP access for approved Web V2.
//! Chrome must validate the real sender origin and a separate website permission.
//! The local user grants an exact (origin, installed provider, status tool) tuple.
//! This is not a remote-profile permission, nor generic provider.call.
use crate::{inbox, mcp_provider};
use anyhow::{bail, Context, Result};
use local_bridge_core::valid_origin;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

const MAX_GRANTS_BYTES: u64 = 64 * 1024;
const MAX_GRANTS: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    origin: String,
    provider: String,
    tool: String,
}

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64
        && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}
fn safe_status_tool(tool: &str) -> bool {
    safe_id(tool) && (tool == "status" || tool.ends_with("_status"))
}
fn checked(origin: &str, provider: &str, tool: &str) -> Result<Grant> {
    if !valid_origin(origin) || !safe_id(provider) || !safe_status_tool(tool) {
        bail!("invalid Web V2 origin or status-only MCP tool");
    }
    Ok(Grant { origin: origin.into(), provider: provider.into(), tool: tool.into() })
}
fn path() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("web-v2-status-grants.json"))
}
fn load() -> Result<Vec<Grant>> {
    let path = path()?;
    if !path.exists() { return Ok(Vec::new()); }
    let info = fs::metadata(&path)?;
    if !info.is_file() || info.len() > MAX_GRANTS_BYTES { bail!("invalid local Web grant file"); }
    let grants: Vec<Grant> = serde_json::from_slice(&fs::read(&path)?)?;
    if grants.len() > MAX_GRANTS { bail!("too many local Web grants"); }
    for grant in &grants { checked(&grant.origin, &grant.provider, &grant.tool)?; }
    Ok(grants)
}
fn save(grants: &[Grant]) -> Result<()> {
    let path = path()?;
    // Directory is private to the OS user (same location as the Bridge inbox).
    let tmp = path.with_extension("json.tmp");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).context("cannot stage Web grant file")?;
    let result = (|| -> Result<()> {
        serde_json::to_writer(&mut file, grants)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        // Windows may reject replacing an existing destination; do not lose
        // old grants or relax execution when this update fails.
        fs::rename(&tmp, &path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(tmp); }
    result
}

/// Only executable from a local CLI with the user's existing MCP registration.
/// A web page or an EAP Profile cannot create its own grants.
pub fn grant_status(origin: &str, provider: &str, tool: &str) -> Result<Value> {
    let grant = checked(origin, provider, tool)?;
    let available = mcp_provider::tools(provider)?;
    if !available["tools"].as_array().is_some_and(|tools| tools.iter().any(|v| v == tool)) {
        bail!("installed MCP provider does not advertise the status tool");
    }
    let mut grants = load()?;
    if !grants.iter().any(|g| g.origin == origin && g.provider == provider && g.tool == tool) {
        if grants.len() >= MAX_GRANTS { bail!("too many Web grants"); }
        grants.push(grant);
        save(&grants)?;
    }
    Ok(json!({"origin":origin,"provider":provider,"tool":tool,
        "granted":true,"scope":"status_only_no_args"}))
}
pub fn revoke_status(origin: &str, provider: &str, tool: &str) -> Result<Value> {
    checked(origin, provider, tool)?;
    let mut grants = load()?;
    grants.retain(|g| !(g.origin == origin && g.provider == provider && g.tool == tool));
    save(&grants)?;
    Ok(json!({"origin":origin,"provider":provider,"tool":tool,"revoked":true}))
}
pub fn capabilities(origin: &str) -> Result<Value> {
    if !valid_origin(origin) { bail!("invalid origin"); }
    let grants: Vec<Value> = load()?.into_iter().filter(|g| g.origin == origin)
        .map(|g| json!({"provider":g.provider,"tool":g.tool,
            "scope":"status_only_no_args"})).collect();
    Ok(json!({"origin":origin,"mcp_status_grants":grants,
        "agent_web_run_supported":false,"profile_trusted":false}))
}
pub fn status_tool(origin: &str, provider: &str, tool: &str) -> Result<Value> {
    checked(origin, provider, tool)?;
    if !load()?.iter().any(|g| g.origin == origin && g.provider == provider && g.tool == tool) {
        bail!("no local grant for this origin and MCP status tool");
    }
    mcp_provider::call(provider, tool, "{}")
}

pub fn handle_native(request: &Value) -> Value {
    let id = request["request_id"].as_str().unwrap_or("");
    let good_id = !id.is_empty() && id.len() <= 80
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'));
    let fail = |error: &str| json!({"protocol_version":2,
        "request_id":if good_id {id} else {""},"ok":false,"error":error});
    if request["protocol_version"] != 2 || !good_id { return fail("invalid_v2_request"); }
    let p = &request["payload"];
    let Some(origin) = p["origin"].as_str() else { return fail("invalid_v2_origin"); };
    if !valid_origin(origin) { return fail("invalid_v2_origin"); }
    let output = match request["action"].as_str() {
        Some("web.v2.capabilities") if p.as_object().is_some_and(|o| o.len() == 1) =>
            capabilities(origin),
        Some("web.v2.mcp.status") if p.as_object().is_some_and(|o| o.len() == 3) => {
            match (p["provider"].as_str(), p["tool"].as_str()) {
                (Some(provider), Some(tool)) => status_tool(origin, provider, tool),
                _ => return fail("invalid_mcp_request"),
            }
        }
        _ => return fail("unsupported_v2_action"),
    };
    match output {
        Ok(result) => json!({"protocol_version":2,"request_id":id,"ok":true,"result":result}),
        Err(_) => fail("not_granted_or_provider_unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_site_and_only_status_shape_is_eligible() {
        assert!(checked("https://app.example.test","wechat","wechat_status").is_ok());
        assert!(checked("https://app.example.test","wechat","wechat_send_message").is_err());
        assert!(checked("https://app.example.test","wechat","wechat_get_messages").is_err());
        assert!(checked("https://app.example.test/path","wechat","wechat_status").is_err());
    }
    #[test]
    fn browser_cannot_run_shell_or_personal_messages() {
        for action in ["web.v2.shell", "web.v2.mcp.call"] {
            let output = handle_native(&json!({"protocol_version":2,"request_id":"task1",
                "action":action,"payload":{"origin":"https://app.example.test"}}));
            assert_eq!(output["ok"],false);
        }
        let output = handle_native(&json!({"protocol_version":2,"request_id":"task1",
            "action":"web.v2.mcp.status",
            "payload":{"origin":"https://app.example.test","provider":"wechat",
                "tool":"wechat_send_message"}}));
        assert_eq!(output["ok"],false);
    }
}
