//! Additive, provider-neutral ApplicationProfile V1 data contract.
//! Validation is structural only: no remote manifest, URL, or capability is
//! trusted merely because it parses. Existing ChannelManifest remains intact.
use crate::{identifier, valid_origin};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationProfile {
    pub schema_version: u32,
    /// Stable instance ID; never derived from a URL, file path, or client version.
    pub profile_id: String,
    pub provider: String,
    pub environment: String,
    pub revision: u64,
    pub expires_at: u64,
    pub application: ApplicationBrand,
    /// Trusted enrollment must separately bind this origin and the user.
    pub origin: String,
    #[serde(default)]
    pub services: BTreeMap<String, String>,
    #[serde(default)]
    pub channels: Vec<ProfileChannel>,
    #[serde(default)]
    pub update: Option<ProfileUpdate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationBrand {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileChannel {
    pub id: String,
    pub title: String,
    pub kind: ProfileChannelKind,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    /// Requested capability IDs, not granted permissions.
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub optional: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileChannelKind {
    Web,
    Native,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileUpdate {
    pub track: String,
    /// Locally pinned update source name, never a URL or a signing key.
    pub source_id: String,
}

fn valid_endpoint(endpoint: &str) -> bool {
    endpoint.len() <= 2048
        && Url::parse(endpoint).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
        })
}

fn valid_logo(profile_origin: &str, value: &str) -> bool {
    if !valid_endpoint(value) {
        return false;
    }
    match (Url::parse(profile_origin), Url::parse(value)) {
        (Ok(profile), Ok(logo)) => {
            logo.scheme() == "https"
                || (logo.scheme() == "http" && logo.origin() == profile.origin())
        }
        _ => false,
    }
}

fn valid_web_entry(value: &str) -> bool {
    value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains("..")
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-'))
}

/// Offline shape validation only; it does NOT enroll, authenticate or activate.
pub fn validate_profile_shape(profile: &ApplicationProfile) -> Result<()> {
    if profile.schema_version != PROFILE_SCHEMA_VERSION
        || !identifier(&profile.profile_id)
        || !identifier(&profile.provider)
        || !identifier(&profile.environment)
        || profile.revision == 0
        || profile.expires_at == 0
        || !valid_origin(&profile.origin)
    {
        bail!("invalid profile version, identity, revision, expiry or origin");
    }
    if !identifier(&profile.application.id)
        || profile.application.name.is_empty()
        || profile.application.name.chars().count() > 100
        || profile.application.name.chars().any(char::is_control)
    {
        bail!("invalid application metadata");
    }
    if profile
        .application
        .logo_url
        .as_deref()
        .is_some_and(|logo| !valid_logo(&profile.origin, logo))
    {
        bail!("invalid logo URL");
    }
    if profile.services.len() > 16
        || profile
            .services
            .iter()
            .any(|(name, url)| !identifier(name) || !valid_endpoint(url))
    {
        bail!("invalid service endpoint declaration");
    }
    if profile.channels.len() > 32 {
        bail!("too many channels");
    }
    if let Some(update) = &profile.update {
        if !identifier(&update.track) || !identifier(&update.source_id) {
            bail!("invalid update track or pinned source ID");
        }
    }
    let mut seen = BTreeSet::new();
    for channel in &profile.channels {
        if !identifier(&channel.id)
            || !seen.insert(&channel.id)
            || channel.title.is_empty()
            || channel.title.chars().count() > 80
            || channel.title.chars().any(char::is_control)
            || channel.capabilities.len() > 16
            || !channel.capabilities.iter().all(|cap| identifier(cap))
        {
            bail!("invalid or duplicate channel");
        }
        match channel.kind {
            ProfileChannelKind::Web => {
                if channel.module.is_some()
                    || !channel.entry.as_deref().is_some_and(valid_web_entry)
                {
                    bail!("web channel requires an origin-relative entry only");
                }
            }
            ProfileChannelKind::Native => {
                if channel.entry.is_some() || !channel.module.as_deref().is_some_and(identifier) {
                    bail!("native channel requires a known module ID only");
                }
            }
        }
    }
    Ok(())
}

/// Enrollment checks freshness separately from the shape and MUST also verify
/// the profile's trusted origin/signature; neither function grants capabilities.
pub fn validate_profile_fresh(profile: &ApplicationProfile, now: u64) -> Result<()> {
    validate_profile_shape(profile)?;
    if profile.expires_at <= now || profile.expires_at > now.saturating_add(7 * 86400) {
        bail!("profile expired or lifetime too long");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn example() -> ApplicationProfile {
        serde_json::from_value(json!({
            "schema_version": 1,
            "profile_id": "sample-prod",
            "provider": "sample",
            "environment": "prod",
            "revision": 1,
            "expires_at": 5000,
            "application": {"id": "sample", "name": "Sample App", "logo_url": "https://assets.example.test/logo.png"},
            "origin": "http://192.0.2.10",
            "services": {"files": "http://192.0.2.10/files"},
            "channels": [
                {"id": "files", "title": "Files", "kind": "native",
                 "module": "cloudfile", "capabilities": ["cloudfile.status"]},
                {"id": "ai", "title": "AI", "kind": "web", "entry": "/ai"}
            ],
            "update": {"track": "stable", "source_id": "trusted-stable"}
        }))
        .unwrap()
    }

    #[test]
    fn accepts_generic_profile_without_activating_it() {
        let x = example();
        validate_profile_shape(&x).unwrap();
        validate_profile_fresh(&x, 4000).unwrap();
        assert_eq!(x.channels.len(), 2);
        assert_eq!(x.profile_id, "sample-prod");
    }

    #[test]
    fn rejects_untrusted_urls_and_remote_commands() {
        let mut x = example();
        x.channels[0].module = Some("powershell -e".into());
        assert!(validate_profile_shape(&x).is_err());
        x.channels[0].module = Some("cloudfile".into());
        x.channels[1].entry = Some("//outside.example/run".into());
        assert!(validate_profile_shape(&x).is_err());
        x.channels[1].entry = Some("/ai".into());
        x.application.logo_url = Some("http://outside.example/logo.png".into());
        assert!(validate_profile_shape(&x).is_err());
        x.application.logo_url = None;
        x.update.as_mut().unwrap().source_id = "https://remote/updates".into();
        assert!(validate_profile_shape(&x).is_err());
    }

    #[test]
    fn rejects_duplicate_capabilities_unknown_fields_and_expired_profiles() {
        let mut x = example();
        x.channels[1].id = "files".into();
        assert!(validate_profile_shape(&x).is_err());
        x.channels[1].id = "ai".into();
        assert!(validate_profile_fresh(&x, 5000).is_err());
        assert!(validate_profile_fresh(&x, 0).is_ok());
        assert!(serde_json::from_str::<ApplicationProfile>(
            r#"{"schema_version":1,"profile_id":"x","unknown":"shell"}"#
        )
        .is_err());
    }
}
