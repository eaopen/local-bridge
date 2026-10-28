//! Generic V2 Agent request: Bridge chooses a real local adapter before work.
//! A Web hint is not proof an Agent is available. No cross-provider retries.
use crate::workbuddy_web;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

fn default_auto() -> String {
    "auto".into()
}
fn default_prefer() -> String {
    "prefer".into()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAgentRequest {
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

fn valid_id(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':' | b'/'))
}
impl RunAgentRequest {
    pub fn check(&self) -> Result<()> {
        if !valid_id(&self.client_key, 80)
            || self.client_key.contains('/')
            || self.client_key.contains(':')
            || self.prompt.trim().is_empty()
            || self.prompt.len() > 12_000
            || self.prompt.contains('\0')
            || !["auto", "workbuddy", "codex", "pi"].contains(&self.agent.as_str())
            || !valid_id(&self.model, 128)
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

fn pick_agent<'a>(requested: &str, mode: &str, candidates: &'a [&'a str]) -> Result<&'a str> {
    if mode == "require" {
        return candidates
            .iter()
            .copied()
            .find(|c| *c == requested)
            .ok_or_else(|| anyhow::anyhow!("required agent unavailable"));
    }
    if requested != "auto" {
        if let Some(selected) = candidates.iter().copied().find(|c| *c == requested) {
            return Ok(selected);
        }
    }
    candidates
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("no available local agent adapter"))
}

/// One task uses one selected adapter. WorkBuddy-only execution at this
/// stage is reported accurately; unverified model/think mode is not applied.
pub fn run(origin: &str, request: RunAgentRequest) -> Result<Value> {
    request.check()?;
    let wb = workbuddy_web::capability(origin)?;
    let available =
        if wb["installed"] == true && wb["local_grant"] == true && wb["run_supported"] == true {
            vec!["workbuddy"]
        } else {
            Vec::new()
        };
    let chosen = pick_agent(&request.agent, &request.agent_mode, &available)?;
    if request.model != "auto" {
        bail!("explicit model not supported by selected adapter");
    }
    if request.think_mode != "auto" {
        bail!("explicit think mode not supported by selected adapter");
    }
    let mut task = match chosen {
        "workbuddy" => workbuddy_web::start(origin, &request.client_key, &request.prompt)?,
        _ => bail!("agent adapter unavailable"),
    };
    task["selected_agent"] = json!(chosen);
    task["requested_agent"] = json!(request.agent);
    task["selection"] = json!(if request.agent == chosen {
        "preferred"
    } else if request.agent == "auto" {
        "automatic"
    } else {
        "fallback_before_execution"
    });
    task["applied_model"] = Value::Null;
    task["applied_think_mode"] = Value::Null;
    Ok(task)
}

pub fn from_value(value: &Value) -> Result<RunAgentRequest> {
    let parsed: RunAgentRequest = serde_json::from_value(value.clone())?;
    parsed.check()?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_selection_respects_preferences_and_strictness() {
        assert_eq!(
            pick_agent("auto", "prefer", &["workbuddy", "codex"]).unwrap(),
            "workbuddy"
        );
        assert_eq!(
            pick_agent("codex", "prefer", &["workbuddy", "codex"]).unwrap(),
            "codex"
        );
        assert_eq!(
            pick_agent("pi", "prefer", &["workbuddy"]).unwrap(),
            "workbuddy"
        );
        assert!(pick_agent("pi", "require", &["workbuddy"]).is_err());
        assert!(pick_agent("auto", "prefer", &[]).is_err());
    }
    #[test]
    fn invalid_options_fail_before_any_task_executes() {
        let base = json!({"client_key":"task-123","prompt":"synthetic task"});
        assert!(from_value(&base).is_ok());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"test",
           "agent":"workbuddy","model":"auto","think_mode":"high",
           "agent_mode":"prefer"}))
        .is_ok());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"test",
           "agent":"auto","agent_mode":"require"}))
        .is_err());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"test",
           "model":"https://secret.example/path?token=abc"}))
        .is_err());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"test",
           "think_mode":"unlimited"}))
        .is_err());
        assert!(from_value(&json!({"client_key":"task-123","prompt":"test",
           "shell":"rm -rf"}))
        .is_err());
    }
}
