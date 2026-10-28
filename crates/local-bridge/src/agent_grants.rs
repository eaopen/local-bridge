//! Per-origin Agent authorization for Web V2. Existing WorkBuddy site grants
//! remain valid; Codex/Pi receive the same explicit OS-user grant semantics.
use crate::{inbox, state_file, workbuddy_web};
use anyhow::{bail, Result};
use local_bridge_core::valid_origin;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct Grant {
    origin: String,
    agent: String,
}
fn path() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("web-v2-agent-grants.json"))
}
fn check(origin: &str, agent: &str) -> Result<()> {
    if !valid_origin(origin) || !["workbuddy", "codex", "pi"].contains(&agent) {
        bail!("invalid Web V2 Agent grant");
    }
    Ok(())
}
fn load() -> Result<Vec<Grant>> {
    let p = path()?;
    if !p.exists() {
        return Ok(Vec::new());
    }
    let info = fs::symlink_metadata(&p)?;
    if !info.file_type().is_file() || info.len() > 16_384 {
        bail!("invalid Agent grant file");
    }
    let grants: Vec<Grant> = serde_json::from_slice(&fs::read(p)?)?;
    if grants.len() > 96 || grants.iter().any(|g| check(&g.origin, &g.agent).is_err()) {
        bail!("invalid Agent grants");
    }
    Ok(grants)
}
fn save(grants: &[Grant]) -> Result<()> {
    state_file::write_json(&path()?, grants)
}

pub fn allowed(origin: &str, agent: &str) -> Result<bool> {
    check(origin, agent)?;
    // Do not break users who previously approved WorkBuddy with grant-workbuddy.
    if agent == "workbuddy" && workbuddy_web::allowed(origin)? {
        return Ok(true);
    }
    Ok(load()?
        .iter()
        .any(|g| g.origin == origin && g.agent == agent))
}
pub fn any(origin: &str) -> Result<bool> {
    if !valid_origin(origin) {
        bail!("invalid site origin");
    }
    Ok(["workbuddy", "codex", "pi"]
        .iter()
        .any(|agent| allowed(origin, agent).unwrap_or(false)))
}
pub fn grant(origin: &str, agent: &str) -> Result<Value> {
    check(origin, agent)?;
    if agent == "workbuddy" {
        // Existing WorkBuddy task permission and data stay authoritative.
        return workbuddy_web::grant(origin);
    }
    state_file::with_lock(&path()?, || {
        let mut grants = load()?;
        if !grants
            .iter()
            .any(|g| g.origin == origin && g.agent == agent)
        {
            if grants.len() >= 96 {
                bail!("too many Agent grants");
            }
            grants.push(Grant {
                origin: origin.into(),
                agent: agent.into(),
            });
            save(&grants)?;
        }
        Ok(())
    })?;
    let mode = if agent == "codex" {
        "read_only_sandbox"
    } else {
        "text_only_no_tools"
    };
    Ok(json!({"origin":origin,"agent":agent,"granted":true,"mode":mode}))
}
pub fn revoke(origin: &str, agent: &str) -> Result<Value> {
    check(origin, agent)?;
    if agent == "workbuddy" {
        return workbuddy_web::revoke(origin);
    }
    state_file::with_lock(&path()?, || {
        let mut grants = load()?;
        grants.retain(|g| !(g.origin == origin && g.agent == agent));
        save(&grants)
    })?;
    Ok(json!({"origin":origin,"agent":agent,"revoked":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_known_providers_and_exact_origins_can_be_granted() {
        assert!(check("https://example.test", "codex").is_ok());
        assert!(check("https://example.test", "pi").is_ok());
        assert!(check("https://example.test", "workbuddy").is_ok());
        assert!(check("https://example.test/path", "pi").is_err());
        assert!(check("https://example.test", "shell").is_err());
    }
}
