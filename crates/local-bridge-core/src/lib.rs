//! The small trust boundary shared by the core, MCP adapter and enterprise
//! integrations. A remote Channel manifest is presentation, NOT code.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use url::Url;

/// Read-only built-in capabilities actually wired into the 0.1 scaffold.
/// A new capability must be implemented before it can be advertised.
pub const AVAILABLE_CAPABILITIES: &[&str] = &["bridge.status", "cloudfile.status"];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelManifest {
    pub schema: u32,
    pub provider: String,
    /// The user's already trusted enterprise instance; HTTP(S) both supported.
    pub origin: String,
    /// Time-bounded configuration. Never treat an expired manifest as fresh.
    pub expires_at: u64,
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub kind: ChannelKind,
    /// Web: an origin-relative path. Builtin: a known capability identifier.
    pub entry: String,
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    Web,
    Builtin,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-' || c == b'_')
}

pub fn valid_origin(source: &str) -> bool {
    Url::parse(source).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https")
            && u.origin().ascii_serialization() == source
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
    })
}

/// Validates *shape* only. Authenticity of a downloaded manifest must be
/// established separately using an authenticated trusted session, and if it
/// travels over HTTP, an enterprise-pinned signature.
pub fn validate_manifest(manifest: &ChannelManifest, now: u64) -> Result<()> {
    if manifest.schema != 1 || !identifier(&manifest.provider) || !valid_origin(&manifest.origin) {
        bail!("unsupported manifest schema, provider or HTTP(S) origin");
    }
    if manifest.expires_at <= now || manifest.expires_at > now.saturating_add(7 * 86400) {
        bail!("channel manifest expired or lifetime too long");
    }
    if manifest.channels.len() > 32 {
        bail!("too many enterprise channels");
    }
    let mut seen = BTreeSet::new();
    for c in &manifest.channels {
        if !identifier(&c.id)
            || !seen.insert(&c.id)
            || c.title.is_empty()
            || c.title.chars().count() > 60
            || c.title.chars().any(char::is_control)
        {
            bail!("invalid or duplicate channel");
        }
        if c.requires.len() > 16 || !c.requires.iter().all(|v| identifier(v)) {
            bail!("invalid required capabilities");
        }
        match c.kind {
            ChannelKind::Web => {
                // Origin-relative URLs only, no redirects to arbitrary hosts,
                // credentials, fragments, query params or traversal.
                if !c.entry.starts_with('/')
                    || c.entry.starts_with("//")
                    || c.entry.contains("..")
                    || c.entry.len() > 256
                    || !c
                        .entry
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-'))
                {
                    bail!("invalid enterprise Web channel path");
                }
            }
            ChannelKind::Builtin => {
                if !identifier(&c.entry) {
                    bail!("invalid built-in capability identifier");
                }
            }
        }
    }
    Ok(())
}

/// Unsupported capabilities stay hidden; an arbitrary manifest never adds a
/// new executable operation to the host.
pub fn visible_channels<'a>(manifest: &'a ChannelManifest, available: &[&str]) -> Vec<&'a Channel> {
    manifest
        .channels
        .iter()
        .filter(|c| {
            c.requires.iter().all(|r| available.contains(&r.as_str()))
                && (c.kind == ChannelKind::Web || available.contains(&c.entry.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> ChannelManifest {
        ChannelManifest {
            schema: 1,
            provider: "etech".into(),
            origin: "http://10.9.8.162".into(),
            expires_at: 5000,
            channels: vec![
                Channel {
                    id: "files".into(),
                    title: "本地文件".into(),
                    kind: ChannelKind::Builtin,
                    entry: "cloudfile.status".into(),
                    requires: vec!["cloudfile.status".into()],
                },
                Channel {
                    id: "ai".into(),
                    title: "企业 AI".into(),
                    kind: ChannelKind::Web,
                    entry: "/agent/ai".into(),
                    requires: vec![],
                },
            ],
        }
    }
    #[test]
    fn accepts_enterprise_http_and_https_without_implicit_downgrade() {
        for url in ["http://10.9.8.162", "https://files.example.test:8443"] {
            let mut x = sample();
            x.origin = url.into();
            validate_manifest(&x, 4000).unwrap();
        }
        for bad in [
            "http://a.example/path",
            "http://evil@a.example",
            "ftp://a.example",
            "https://a.example/",
            "http://a.example?x=1",
        ] {
            assert!(!valid_origin(bad), "{bad}");
        }
    }
    #[test]
    fn rejects_remote_executables_traversal_and_unknown_fields() {
        let mut m = sample();
        m.channels[1].entry = "//evil.example/run".into();
        assert!(validate_manifest(&m, 4000).is_err());
        m.channels[1].entry = "/../unsafe".into();
        assert!(validate_manifest(&m, 4000).is_err());
        m.channels[1].entry = "/agent/ai".into();
        m.channels[0].kind = ChannelKind::Builtin;
        m.channels[0].entry = "powershell -e".into();
        assert!(validate_manifest(&m, 4000).is_err());
        assert!(serde_json::from_str::<ChannelManifest>(r#"{"schema":1,"extra":"run"}"#).is_err());
    }
    #[test]
    fn unknown_capability_is_not_exposed_to_user() {
        let mut m = sample();
        m.channels.push(Channel {
            id: "shell".into(),
            title: "Hidden".into(),
            kind: ChannelKind::Builtin,
            entry: "shell.run".into(),
            requires: vec![],
        });
        validate_manifest(&m, 4000).unwrap();
        let visible = visible_channels(&m, AVAILABLE_CAPABILITIES);
        assert_eq!(visible.len(), 2);
        assert!(!visible.iter().any(|c| c.id == "shell"));
    }
    #[test]
    fn expiry_duplicate_and_malformed_json_are_rejected() {
        let mut m = sample();
        assert!(validate_manifest(&m, 5001).is_err());
        m.expires_at = 5000;
        m.channels[1].id = "files".into();
        assert!(validate_manifest(&m, 4000).is_err());
    }
}
