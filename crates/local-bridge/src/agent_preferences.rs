//! User-owned defaults for the unified V2 Agent selection. The browser and
//! enterprise Profile cannot write this file; only the OS user's CLI/Desktop
//! can update it. Per-origin grants and provider capabilities still win.
use crate::{inbox, state_file};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 4096;
fn default_auto() -> String {
    "auto".into()
}
fn default_prefer() -> String {
    "prefer".into()
}
fn default_priority() -> Vec<String> {
    ["workbuddy", "codex", "pi"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
fn schema_version() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentPreferences {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_auto")]
    pub agent: String,
    #[serde(default = "default_auto")]
    pub model: String,
    #[serde(default = "default_auto")]
    pub think_mode: String,
    #[serde(default = "default_prefer")]
    pub agent_mode: String,
    #[serde(default = "default_priority")]
    pub priority: Vec<String>,
}
impl Default for AgentPreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            agent: default_auto(),
            model: default_auto(),
            think_mode: default_auto(),
            agent_mode: default_prefer(),
            priority: default_priority(),
        }
    }
}
impl AgentPreferences {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1
            || !["auto", "workbuddy", "codex", "pi"].contains(&self.agent.as_str())
            || !["prefer", "require"].contains(&self.agent_mode.as_str())
            || (self.agent == "auto" && self.agent_mode == "require")
            || !["auto", "off", "low", "medium", "high", "xhigh", "max"]
                .contains(&self.think_mode.as_str())
            || self.model.is_empty()
            || self.model.len() > 128
            || self.model.contains("://")
            || self.model.starts_with('/')
            || self.model.contains("..")
            || !self
                .model
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b':' | b'/'))
            || self.priority.len() != 3
            || ["workbuddy", "codex", "pi"].iter().any(|name| {
                self.priority
                    .iter()
                    .filter(|entry| entry.as_str() == *name)
                    .count()
                    != 1
            })
            || (self.agent == "workbuddy"
                && !crate::agent_options::options_supported(
                    "workbuddy",
                    &self.model,
                    &self.think_mode,
                ))
        {
            bail!("invalid Agent preferences");
        }
        Ok(())
    }
}
fn file_path() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("agent-preferences-v1.json"))
}
fn load_from(path: &Path) -> Result<AgentPreferences> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AgentPreferences::default())
        }
        Err(error) => return Err(error.into()),
    };
    if !meta.file_type().is_file() || meta.len() > MAX_BYTES {
        bail!("invalid Agent preferences file");
    }
    let value: AgentPreferences = serde_json::from_slice(&fs::read(path)?)?;
    value.validate()?;
    Ok(value)
}
pub fn load() -> Result<AgentPreferences> {
    load_from(&file_path()?)
}
fn save_to(path: &Path, prefs: &AgentPreferences) -> Result<()> {
    prefs.validate()?;
    state_file::with_lock(path, || state_file::write_json(path, prefs))
}
pub fn save(prefs: AgentPreferences) -> Result<AgentPreferences> {
    save_to(&file_path()?, &prefs)?;
    Ok(prefs)
}
pub fn save_value(value: Value) -> Result<Value> {
    let prefs: AgentPreferences = serde_json::from_value(value)?;
    Ok(serde_json::to_value(save(prefs)?)?)
}
pub fn get_value() -> Result<Value> {
    Ok(serde_json::to_value(load()?)?)
}

/// Apply user defaults only to omitted request fields. An explicit "auto"
/// remains the caller's selection, not a signal to override it. No grant,
/// authentication, task id, prompt or model credential comes from preferences.
pub fn apply_defaults(value: &Value, prefs: &AgentPreferences) -> Result<Value> {
    prefs.validate()?;
    let mut map: Map<String, Value> = value
        .as_object()
        .context("Agent V2 request must be a JSON object")?
        .clone();
    let explicit_agent = map.contains_key("agent");
    if !explicit_agent {
        map.insert("agent".into(), json!(prefs.agent));
    }
    if !map.contains_key("model") {
        map.insert("model".into(), json!(prefs.model));
    }
    if !map.contains_key("think_mode") {
        map.insert("think_mode".into(), json!(prefs.think_mode));
    }
    if !map.contains_key("agent_mode") {
        // Explicit caller agent choices should not inherit a saved require
        // policy for an unrelated preferred agent.
        map.insert(
            "agent_mode".into(),
            json!(if explicit_agent {
                "prefer"
            } else {
                prefs.agent_mode.as_str()
            }),
        );
    }
    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn omitted_fields_use_defaults_but_explicit_auto_wins() {
        let prefs = AgentPreferences {
            agent: "codex".into(),
            model: "gpt-test".into(),
            think_mode: "high".into(),
            agent_mode: "require".into(),
            priority: vec!["pi".into(), "codex".into(), "workbuddy".into()],
            ..Default::default()
        };
        prefs.validate().unwrap();
        let base = json!({"api_version":2, "client_key":"test-1","prompt":"hello"});
        let filled = apply_defaults(&base, &prefs).unwrap();
        assert_eq!(filled["agent"], "codex");
        assert_eq!(filled["agent_mode"], "require");
        assert_eq!(filled["think_mode"], "high");
        let explicit = apply_defaults(
            &json!({
                "api_version":2,"client_key":"test-2","prompt":"hello",
                "agent":"auto","think_mode":"auto","model":"auto"
            }),
            &prefs,
        )
        .unwrap();
        assert_eq!(explicit["agent"], "auto");
        assert_eq!(explicit["agent_mode"], "prefer");
        assert_eq!(explicit["model"], "auto");
        assert_eq!(explicit["think_mode"], "auto");
    }
    #[test]
    fn bad_policy_is_rejected_without_weakening_existing_authorization() {
        let mut p = AgentPreferences {
            priority: vec!["codex".into(), "codex".into(), "pi".into()],
            ..Default::default()
        };
        assert!(p.validate().is_err());
        p = AgentPreferences::default();
        p.agent = "workbuddy".into();
        p.think_mode = "off".into();
        assert!(p.validate().is_err());
        p = AgentPreferences::default();
        p.model = "../private".into();
        assert!(p.validate().is_err());
        assert!(serde_json::from_value::<AgentPreferences>(
            json!({"agent":"codex","api_key":"secret"})
        )
        .is_err());
    }
    #[test]
    fn saved_json_remains_atomic_private_and_backward_compatible() {
        let name = format!(
            "bridge-preferences-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(name);
        let path = root.join("agent-preferences-v1.json");
        assert_eq!(load_from(&path).unwrap(), AgentPreferences::default());
        let p = AgentPreferences {
            priority: vec!["pi".into(), "codex".into(), "workbuddy".into()],
            ..Default::default()
        };
        save_to(&path, &p).unwrap();
        assert_eq!(load_from(&path).unwrap(), p);
        fs::write(&path, b"broken").unwrap();
        assert!(load_from(&path).is_err()); // no silent reset or grant
        fs::remove_dir_all(root).unwrap();
    }
}
