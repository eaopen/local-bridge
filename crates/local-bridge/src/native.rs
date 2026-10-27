//! Optional Chrome Native Messaging Host V1 for the independent Local Tool
//! Bridge extension. This is NOT the line-based MCP stdio transport.
//!
//! Only user-initiated extension popup context is accepted. The real handoff
//! is a local user-owned inbox item, NOT an AI/file operation completion.
use crate::inbox;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

const MAX_REQUEST: usize = 128 * 1024;
const MAX_RESPONSE: usize = 256 * 1024;
const HOST_NAME: &str = "com.eaopen.browser_bridge";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserContext {
    title: String,
    url: String,
    selection: String,
}

fn valid_request_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

fn validate_context(value: &Value) -> Result<Value> {
    if serde_json::to_vec(value)?.len() > 64 * 1024 {
        bail!("payload_too_large");
    }
    let context: BrowserContext =
        serde_json::from_value(value.clone()).context("invalid_context")?;
    if context.title.chars().count() > 300
        || context.url.len() > 2000
        || context.selection.trim().is_empty()
        || context.selection.chars().count() > 16000
    {
        bail!("invalid_context");
    }
    if !context.url.is_empty() {
        let url = url::Url::parse(&context.url).context("invalid_context")?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("invalid_context");
        }
    }
    Ok(json!({
        "title":context.title,
        "url":context.url,
        "selection":context.selection
    }))
}

pub fn handle(request: &Value) -> Value {
    let id = request["request_id"].as_str().unwrap_or("");
    let public_id = if valid_request_id(id) { id } else { "" };
    let failure = |error: &'static str| {
        json!({
            "protocol_version":1,"request_id":public_id,"ok":false,"error":error
        })
    };
    if request["protocol_version"] != 1 || !valid_request_id(id) {
        return failure("protocol_or_request_id_invalid");
    }
    match request["action"].as_str() {
        Some("host.status") => {
            if request["payload"].as_object().is_none_or(|v| !v.is_empty()) {
                return failure("invalid_payload");
            }
            json!({"protocol_version":1,"request_id":id,"ok":true,"result":{
                "provider":"local-bridge",
                "connected_to_local_bridge":true,
                "native_protocol":1,
                "capabilities":["host.status","context.share"],
                "handoff":"local-inbox",
                "ai_run_enabled":false
            }})
        }
        Some("context.share") => {
            let Some(payload) = request.get("payload") else {
                return failure("invalid_context");
            };
            let Ok(ctx) = validate_context(payload) else {
                return failure("invalid_context");
            };
            match inbox::save(ctx) {
                Ok(task_id) => json!({
                    "protocol_version":1,"request_id":id,"ok":true,
                    "result":{
                        "provider":"local-bridge",
                        "received":true,
                        "stored":true,
                        "forwarded":false,
                        "status":"received",
                        "task_id":task_id,
                        "note":"Saved to the local user inbox; no AI or file task was started."
                    }
                }),
                Err(_) => failure("local_inbox_unavailable"),
            }
        }
        _ => failure("unsupported_action"),
    }
}

fn read_frame(input: &mut impl Read) -> Result<Value> {
    let mut prefix = [0_u8; 4];
    input
        .read_exact(&mut prefix)
        .context("missing Native Messaging length")?;
    let length = u32::from_ne_bytes(prefix) as usize;
    if length == 0 || length > MAX_REQUEST {
        bail!("invalid Native Messaging length");
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .context("truncated Native Messaging request")?;
    serde_json::from_slice(&bytes).context("invalid Native Messaging JSON")
}

fn write_frame(output: &mut impl Write, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_RESPONSE {
        bail!("Native Messaging response too large");
    }
    output.write_all(&(bytes.len() as u32).to_ne_bytes())?;
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
}

/// Chrome sendNativeMessage starts this binary separately for each request.
pub fn serve() -> Result<()> {
    let request = read_frame(&mut io::stdin().lock())?;
    let reply = handle(&request);
    write_frame(&mut io::stdout().lock(), &reply)
}

fn chrome_manifest_dir() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        Ok(inbox::data_root()?.join("NativeMessagingHosts"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home)
            .join("Library/Application Support/Google/Chrome/NativeMessagingHosts"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home).join(".config/google-chrome/NativeMessagingHosts"))
    }
}

pub fn install_host(extension_id: &str, replace: bool) -> Result<Value> {
    if extension_id.len() != 32 || !extension_id.bytes().all(|b| (b'a'..=b'p').contains(&b)) {
        bail!("invalid Chrome extension id (expected 32 letters a-p)");
    }
    let current = env::current_exe().context("cannot identify Local Bridge CLI")?;
    let executable = if cfg!(windows) {
        "local-bridge-native-host.exe"
    } else {
        "local-bridge-native-host"
    };
    let native = current.with_file_name(executable);
    if !native.is_file() {
        bail!("native host executable is missing; build/install both Local Bridge binaries first");
    }

    let dir = chrome_manifest_dir()?;
    fs::create_dir_all(&dir).context("cannot create NativeMessagingHosts directory")?;
    let manifest_path = dir.join(format!("{HOST_NAME}.json"));
    let manifest = json!({
        "name":HOST_NAME,
        "description":"Optional Local Bridge native provider for Local Tool Bridge",
        "path":native.canonicalize()?.to_string_lossy(),
        "type":"stdio",
        "allowed_origins":[format!("chrome-extension://{extension_id}/")]
    });
    let data = serde_json::to_vec_pretty(&manifest)?;
    if manifest_path.exists() && fs::read(&manifest_path)? != data && !replace {
        bail!("Native host manifest exists with different settings; use --replace to opt in to updating it");
    }
    fs::write(&manifest_path, data).context("cannot write Chrome host manifest")?;

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let registry = format!("HKCU\\Software\\Google\\Chrome\\NativeMessagingHosts\\{HOST_NAME}");
        let status = Command::new("reg.exe")
            .args(["ADD", &registry, "/ve", "/t", "REG_SZ", "/d"])
            .arg(&manifest_path)
            .arg("/f")
            .status()
            .context("cannot register Chrome native host")?;
        if !status.success() {
            bail!("Windows native host registry registration failed");
        }
    }

    Ok(json!({
        "installed":true,
        "host":HOST_NAME,
        "extension_id":extension_id,
        "manifest":manifest_path.to_string_lossy(),
        "restart_chrome_if_needed":true
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reports_only_actual_native_capabilities() {
        let answer = handle(&json!({
            "protocol_version":1,"request_id":"abc-1",
            "action":"host.status","payload":{}
        }));
        assert_eq!(answer["ok"], true);
        assert_eq!(answer["result"]["provider"], "local-bridge");
        assert_eq!(answer["result"]["ai_run_enabled"], false);
        assert_eq!(answer["result"]["connected_to_local_bridge"], true);
        assert_eq!(
            handle(&json!({
                "protocol_version":1,"request_id":"abc-2",
                "action":"shell.run","payload":{}
            }))["ok"],
            false
        );
    }

    #[test]
    fn rejects_untrusted_context_and_invalid_requests() {
        assert!(validate_context(&json!({
            "title":"Note","url":"https://example.org/page?token=secret","selection":"hello"
        }))
        .is_err());
        assert!(validate_context(&json!({
            "title":"Note","url":"https://example.org","selection":""
        }))
        .is_err());
        assert!(validate_context(&json!({
            "title":"Note","url":"","selection":"user-selected note"
        }))
        .is_ok());
        assert_eq!(
            handle(&json!({
                "protocol_version":1,"request_id":"../../bad",
                "action":"host.status","payload":{}
            }))["ok"],
            false
        );
    }

    #[test]
    fn native_framing_roundtrip_and_limits() {
        let request = json!({
            "protocol_version":1,"request_id":"r1","action":"host.status","payload":{}
        });
        let mut frame = Vec::new();
        write_frame(&mut frame, &request).unwrap();
        let restored = read_frame(&mut Cursor::new(frame)).unwrap();
        assert_eq!(request, restored);
        let bad = (MAX_REQUEST as u32 + 1).to_ne_bytes();
        assert!(read_frame(&mut Cursor::new(bad)).is_err());
    }
}
