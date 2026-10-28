//! Optional Chrome Native Messaging Host V1 for the independent Local Tool
//! Bridge extension. This is NOT the line-based MCP stdio transport.
//!
//! Only user-initiated extension popup context is accepted. The real handoff
//! is a local user-owned inbox item, NOT an AI/file operation completion.
use crate::{agent, inbox, web_v2};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use url::Url;

const MAX_REQUEST: usize = 128 * 1024;
const MAX_RESPONSE: usize = 256 * 1024;
const MAX_SUMMARY_BYTES: usize = 48 * 1024;
const HOST_NAME: &str = "com.eaopen.browser_bridge";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelaySource {
    title: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayReturn {
    relay_id: String,
    #[serde(default)]
    task_ref: String,
    web_ai: String,
    source: RelaySource,
    reply: BrowserContext,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserContext {
    title: String,
    url: String,
    selection: String,
}

/// Only the Chrome service worker may construct 'origin' from sender.origin.
/// The original enterprise service must sign/verify the short-lived ticket.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnterpriseAiRequest {
    origin: String,
    api_path: String,
    model: String,
    ticket: String,
    prompt: String,
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

fn validate_relay(value: &Value) -> Result<(Value, Value)> {
    if serde_json::to_vec(value)?.len() > 64 * 1024 {
        bail!("payload_too_large");
    }
    let raw: RelayReturn = serde_json::from_value(value.clone()).context("invalid_relay")?;
    if raw.relay_id.len() != 36
        || !raw.relay_id.starts_with("rel-")
        || !raw.relay_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        bail!("invalid_relay_id");
    }
    if raw.web_ai.is_empty()
        || raw.web_ai.len() > 32
        || !raw
            .web_ai
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'-')
    {
        bail!("invalid_provider_name");
    }
    if raw.task_ref.len() > 96
        || !raw
            .task_ref
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'.' | b'-' | b'_'))
    {
        bail!("invalid_task_ref");
    }
    let source = validate_context(&json!({
        "title":raw.source.title,"url":raw.source.url,"selection":"source"
    }))?;
    let reply = validate_context(&json!({
        "title":raw.reply.title,
        "url":raw.reply.url,
        "selection":raw.reply.selection
    }))?;
    let relation = json!({
        "relay_id":raw.relay_id,
        "task_ref":raw.task_ref,
        "web_ai":raw.web_ai,
        "source":{"title":source["title"],"url":source["url"]}
    });
    Ok((reply, relation))
}

fn validate_enterprise_ai(value: &Value) -> Result<(String, String, String, String)> {
    if serde_json::to_vec(value)?.len() > 96 * 1024 {
        bail!("enterprise request too large");
    }
    let raw: EnterpriseAiRequest =
        serde_json::from_value(value.clone()).context("invalid enterprise request")?;
    let origin = Url::parse(&raw.origin).context("invalid enterprise origin")?;
    let local_http = origin.scheme() == "http"
        && matches!(origin.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if (origin.scheme() != "https" && !local_http)
        || !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
        || raw.origin != origin.origin().ascii_serialization()
    {
        bail!("invalid enterprise origin");
    }
    if !raw.api_path.starts_with('/')
        || raw.api_path.starts_with("//")
        || raw.api_path.len() > 128
        || raw.api_path.contains("..")
        || !raw
            .api_path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-'))
    {
        bail!("invalid same-origin enterprise API path");
    }
    let endpoint = origin.join(&raw.api_path)?;
    if endpoint.origin() != origin.origin() {
        bail!("enterprise API path cannot leave the approved Web origin");
    }
    if raw.model.is_empty()
        || raw.model.len() > 128
        || raw.model.contains("..")
        || !raw
            .model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'/' | b'.'))
        || raw.ticket.trim().is_empty()
        || raw.ticket.len() > 8192
        || raw
            .ticket
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b < 0x21)
        || raw.prompt.trim().is_empty()
        || raw.prompt.len() > 64 * 1024
    {
        bail!("invalid enterprise model, authorization or prompt");
    }
    Ok((endpoint.to_string(), raw.model, raw.ticket, raw.prompt))
}

pub fn handle(request: &Value) -> Value {
    if request["protocol_version"] == 2 {
        return web_v2::handle_native(request);
    }
    handle_with_runners(request, agent::run_codex, agent::run_pi_enterprise_ticket)
}

// Keep existing Codex test injection while the approved enterprise path can
// be separately tested without invoking any network or account.
#[cfg(test)]
fn handle_with_summary(request: &Value, runner: impl FnOnce(&str) -> Result<Value>) -> Value {
    handle_with_runners(request, runner, agent::run_pi_enterprise_ticket)
}

fn handle_with_runners(
    request: &Value,
    summarize: impl FnOnce(&str) -> Result<Value>,
    enterprise: impl FnOnce(&str, &str, &str, &str) -> Result<Value>,
) -> Value {
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
                "capabilities":["host.status","context.share","context.summarize","relay.return","enterprise.pi.run","web.v2.capabilities","web.v2.mcp.status"],
                "handoff":"local-inbox",
                "ai_run_supported":true,
                "ai_run_enabled":false,
                "ai_run_mode":"on_explicit_popup_request",
                "agent_readiness":"not_checked; Codex must be installed and logged in"
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
        Some("relay.return") => {
            // A human-selected Web AI reply is DATA, not a tool call.
            // Store it in the existing inbox with an optional task reference.
            let Some(payload) = request.get("payload") else {
                return failure("invalid_relay");
            };
            let Ok((reply, relation)) = validate_relay(payload) else {
                return failure("invalid_relay");
            };
            match inbox::save_relay(reply, relation.clone()) {
                Ok(task_id) => json!({
                    "protocol_version":1,"request_id":id,"ok":true,
                    "result":{
                        "provider":"local-bridge",
                        "received":true,
                        "stored":true,
                        "forwarded":false,
                        "status":"received",
                        "task_id":task_id,
                        "relay_id":relation["relay_id"],
                        "related_task_ref":relation["task_ref"],
                        "note":"Web AI reply stored for human review; no local tool was executed."
                    }
                }),
                Err(_) => failure("local_inbox_unavailable"),
            }
        }
        Some("enterprise.pi.run") => {
            // The extension must require an *additional per-origin user grant*
            // and derive origin from Chrome's trusted sender, never from a page.
            // Local tools are disabled; only one enterprise AI text task runs.
            let Some(payload) = request.get("payload") else {
                return failure("invalid_enterprise_request");
            };
            let Ok((base, model, ticket, prompt)) = validate_enterprise_ai(payload) else {
                return failure("invalid_enterprise_request");
            };
            match enterprise(&base, &model, &ticket, &prompt) {
                Ok(reply) => {
                    let answer = reply["result"].as_str().unwrap_or("");
                    if reply["status"] != "completed" || answer.trim().is_empty() {
                        return failure("enterprise_ai_incomplete");
                    }
                    if answer.len() > MAX_SUMMARY_BYTES {
                        return failure("enterprise_ai_response_too_large");
                    }
                    json!({
                        "protocol_version":1,"request_id":id,"ok":true,
                        "result":{
                            "provider":"local-bridge",
                            "agent":"pi",
                            "model_source":"enterprise_session",
                            "status":"completed",
                            "result":answer,
                            "tools_enabled":false,
                            "local_files_changed":false
                        }
                    })
                }
                Err(_) => failure("enterprise_ai_unavailable_or_failed"),
            }
        }
        Some("context.summarize") => {
            // Only the trusted extension popup can send this typed action;
            // the extension service worker explicitly rejects website calls.
            // No arbitrary prompt, executable, model or tool name is accepted.
            let Some(payload) = request.get("payload") else {
                return failure("invalid_context");
            };
            let Ok(ctx) = validate_context(payload) else {
                return failure("invalid_context");
            };
            let Ok(prompt) = inbox::summarize_context(&ctx) else {
                return failure("invalid_context");
            };
            match summarize(&prompt) {
                Ok(result) => {
                    let answer = result["result"].as_str().unwrap_or("");
                    if result["status"] != "completed" || answer.trim().is_empty() {
                        return failure("agent_incomplete");
                    }
                    if answer.len() > MAX_SUMMARY_BYTES {
                        return failure("agent_result_too_large");
                    }
                    json!({
                        "protocol_version":1,"request_id":id,"ok":true,
                        "result":{
                            "provider":"local-bridge",
                            "agent":"codex",
                            "status":"completed",
                            "summary":answer,
                            "stored":false,
                            "forwarded_to_agent":true,
                            "sent_by_user_click":true,
                            "note":"User invoked their existing Codex CLI; the model backend is configured in Codex."
                        }
                    })
                }
                Err(_) => failure("agent_unavailable_or_failed"),
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
        assert_eq!(answer["result"]["ai_run_supported"], true);
        assert_eq!(answer["result"]["ai_run_enabled"], false);
        assert_eq!(answer["result"]["ai_run_mode"], "on_explicit_popup_request");
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
    fn explicit_popup_summary_uses_fixed_prompt_and_reports_real_completion() {
        use std::cell::Cell;
        let called = Cell::new(false);
        let reply = handle_with_summary(
            &json!({
                "protocol_version":1,"request_id":"sum_1",
                "action":"context.summarize",
                "payload":{"title":"Project","url":"https://example.org/page",
                           "selection":"Please summarize today's project notes"}
            }),
            |prompt| {
                called.set(true);
                assert!(prompt.contains("untrusted data"));
                assert!(prompt.contains("summarize"));
                assert!(prompt.contains("Please summarize today's project notes"));
                Ok(json!({"status":"completed","result":"Two useful points."}))
            },
        );
        assert!(called.get());
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"]["summary"], "Two useful points.");
        assert_eq!(reply["result"]["stored"], false);

        let rejected = handle_with_summary(
            &json!({
                "protocol_version":1,"request_id":"sum_2","action":"context.summarize",
                "payload":{"title":"t","url":"https://example.org/?secret=x","selection":"x"}
            }),
            |_| panic!("must reject invalid context before starting a model"),
        );
        assert_eq!(rejected["ok"], false);
        assert_eq!(rejected["error"], "invalid_context");

        let failed = handle_with_summary(
            &json!({
                "protocol_version":1,"request_id":"sum_3","action":"context.summarize",
                "payload":{"title":"t","url":"","selection":"x"}
            }),
            |_| anyhow::bail!("Codex not installed"),
        );
        assert_eq!(failed["ok"], false);
        assert_eq!(failed["error"], "agent_unavailable_or_failed");
    }

    #[test]
    fn relay_return_requires_typed_manual_reply() {
        let good = json!({
            "relay_id":"rel-0123456789abcdef0123456789abcdef",
            "task_ref":"ctx-abcdef012345",
            "web_ai":"chatgpt",
            "source":{"title":"Original task","url":"https://example.org/article"},
            "reply":{"title":"ChatGPT","url":"https://chatgpt.com",
                     "selection":"Proposed steps (not executed)."}
        });
        let (reply, relation) = validate_relay(&good).unwrap();
        assert_eq!(reply["selection"], "Proposed steps (not executed).");
        assert_eq!(relation["task_ref"], "ctx-abcdef012345");
        assert_eq!(relation["source"]["url"], "https://example.org/article");
        let mut bad = good.clone();
        bad["reply"]["url"] = json!("https://chatgpt.com/?token=secret");
        assert!(validate_relay(&bad).is_err());
        bad = good.clone();
        bad["relay_id"] = json!("../secrets");
        assert!(validate_relay(&bad).is_err());
        bad = good.clone();
        bad["task_ref"] = json!("../secret");
        assert!(validate_relay(&bad).is_err());
    }

    #[test]
    fn enterprise_ai_is_same_origin_and_requires_trusted_ticket() {
        let valid = json!({
            "origin":"https://app.example.org",
            "api_path":"/api/local-ai/v1",
            "model":"team-chat",
            "ticket":"short-lived-test-ticket",
            "prompt":"Return only TEST_OK"
        });
        let (base, model, ticket, prompt) = validate_enterprise_ai(&valid).unwrap();
        assert_eq!(base, "https://app.example.org/api/local-ai/v1");
        assert_eq!(model, "team-chat");
        assert_eq!(ticket, "short-lived-test-ticket");
        assert_eq!(prompt, "Return only TEST_OK");
        let reply = handle_with_runners(
            &json!({"protocol_version":1,"request_id":"enterprise_1",
                "action":"enterprise.pi.run","payload":valid}),
            |_| panic!("must not invoke Codex for an enterprise Pi request"),
            |endpoint, model, token, _| {
                assert_eq!(endpoint, "https://app.example.org/api/local-ai/v1");
                assert_eq!(model, "team-chat");
                assert_eq!(token, "short-lived-test-ticket");
                Ok(json!({"status":"completed","result":"TEST_OK"}))
            },
        );
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["result"]["result"], "TEST_OK");
        assert_eq!(reply["result"]["tools_enabled"], false);
        let bad_cases = [
            ("origin", "https://app.example.org/evil"),
            ("origin", "http://public.example.org"),
            ("api_path", "//evil.example.org/v1"),
            ("api_path", "/api/../evil"),
            ("api_path", "/api/v1?redirect=bad"),
            ("ticket", " "),
            ("prompt", ""),
        ];
        for (field, value) in bad_cases {
            let mut bad = valid.clone();
            bad[field] = json!(value);
            assert!(validate_enterprise_ai(&bad).is_err(), "{field}={value}");
        }
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
