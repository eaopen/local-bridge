//! Single-task, enterprise-provided Pi model binding.
//!
//! The already-authenticated enterprise application owns the model endpoint,
//! user authorization, short-lived session token, billing and data policy.
//! Local Bridge neither prompts the OS user for model credentials nor copies
//! their normal Pi settings. This module never downloads or executes code.
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

pub const PROVIDER: &str = "local-bridge-enterprise";
pub const TOKEN_ENV: &str = "LOCAL_BRIDGE_ENTERPRISE_AI_TOKEN";
const URL_ENV: &str = "LOCAL_BRIDGE_ENTERPRISE_AI_BASE_URL";
const MODEL_ENV: &str = "LOCAL_BRIDGE_ENTERPRISE_AI_MODEL";

/// Per-invocation Pi config, not installed into ~/.pi/agent. No token is
/// written to disk. Drop removes this temporary configuration after Pi exits.
pub struct EnterprisePiSession {
    pub model: String,
    pub agent_dir: PathBuf,
}

impl Drop for EnterprisePiSession {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.agent_dir);
    }
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 128
        && !model.starts_with('-')
        && !model.contains("..")
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'/' | b'.'))
}

fn check_endpoint(base_url: &str) -> Result<()> {
    let url = Url::parse(base_url).context("enterprise AI endpoint URL is invalid")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        bail!("enterprise AI endpoint must not contain credentials, query or fragment");
    }
    let secure = url.scheme() == "https";
    let local_http = url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !secure && !local_http {
        bail!("enterprise AI endpoint requires HTTPS or a local loopback test endpoint");
    }
    Ok(())
}

impl EnterprisePiSession {
    /// In production these variables must be supplied by a *trusted* existing
    /// enterprise session/binding, not arbitrary Web page data or model text.
    /// Missing authorization means unavailable, never silent credential reuse.
    pub fn from_environment() -> Result<Self> {
        let base = env::var(URL_ENV).context("enterprise AI session unavailable: no endpoint")?;
        let model = env::var(MODEL_ENV).context("enterprise AI session unavailable: no model")?;
        let token =
            env::var(TOKEN_ENV).context("enterprise AI session unavailable: no authorization")?;
        Self::from_values(&base, &model, &token)
    }

    pub(crate) fn from_values(base: &str, model: &str, token: &str) -> Result<Self> {
        check_endpoint(base)?;
        if !valid_model(model) {
            bail!("enterprise AI model identifier is invalid");
        }
        if token.trim().is_empty() || token.len() > 8192 {
            bail!("enterprise AI session authorization is unavailable or invalid");
        }
        let content = json!({
            "providers": {
                PROVIDER: {
                    "baseUrl": base,
                    "api": "openai-completions",
                    // Pi resolves the token from the *child process environment*.
                    // The user's long-term credentials are never stored by Bridge.
                    "apiKey": "$LOCAL_BRIDGE_ENTERPRISE_AI_TOKEN",
                    "models": [{
                        "id": model,
                        "name": "Enterprise AI",
                        "reasoning": false,
                        "input": ["text"],
                        "contextWindow": 8192,
                        "maxTokens": 2048,
                        // Enterprise owns actual server-side accounting and billing.
                        "cost": {"input": 0,"output": 0,"cacheRead": 0,"cacheWrite": 0}
                    }]
                }
            }
        });
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("invalid system clock")?
            .as_nanos();
        for n in 0..16 {
            let dir = env::temp_dir().join(format!(
                "local-bridge-enterprise-pi-{}-{now:x}-{n}",
                std::process::id()
            ));
            match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).context("cannot create temporary Pi configuration"),
            }
            let session = Self {
                model: model.to_string(),
                agent_dir: dir,
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&session.agent_dir, fs::Permissions::from_mode(0o700))?;
            }
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(session.agent_dir.join("models.json"))
                .context("cannot create isolated Pi models.json")?;
            serde_json::to_writer(&mut file, &content)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            return Ok(session);
        }
        bail!("cannot allocate a unique temporary Pi directory");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enterprise_model_is_private_and_never_serializes_bearer_token() {
        let session = EnterprisePiSession::from_values(
            "https://ai.example.invalid/openai/v1",
            "corp-chat",
            "private-dummy-test-token",
        )
        .unwrap();
        let stored = fs::read_to_string(session.agent_dir.join("models.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            parsed["providers"]["local-bridge-enterprise"]["models"][0]["id"],
            "corp-chat"
        );
        assert!(stored.contains("corp-chat"));
        assert!(stored.contains("$LOCAL_BRIDGE_ENTERPRISE_AI_TOKEN"));
        assert!(!stored.contains("private-dummy-test-token"));
        assert!(!stored.contains("~/.pi"));
        let path = session.agent_dir.clone();
        drop(session);
        assert!(!path.exists());
    }

    #[test]
    fn rejects_untrusted_urls_and_missing_authorization() {
        for endpoint in [
            "http://example.org/v1",
            "file:///etc/passwd",
            "https://person:password@example.org/v1",
            "https://example.org/v1?token=secret",
            "https://example.org/v1#frag",
        ] {
            assert!(check_endpoint(endpoint).is_err(), "{endpoint}");
        }
        assert!(check_endpoint("http://127.0.0.1:32123/v1").is_ok());
        assert!(check_endpoint("https://enterprise.example.org/v1").is_ok());
        assert!(EnterprisePiSession::from_values(
            "https://enterprise.example.org/v1",
            "corp-chat",
            ""
        )
        .is_err());
        assert!(EnterprisePiSession::from_values(
            "https://enterprise.example.org/v1",
            "../unsafe",
            "test"
        )
        .is_err());
    }
}
