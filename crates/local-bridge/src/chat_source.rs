//! Native-user initiated, bounded chat reads from an optional local MCP provider.
//! No sends, automatic probes or AI calls. The provider is user-installed and
//! owns authentication, message storage, side effects and receipts. Bridge only
//! selects one provider, validates the returned source/identity and degrades.
use crate::mcp_provider;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

const SOURCES: [&str; 4] = ["local_index", "desktop_ui", "structured_db", "web_page"];
// Web-page providers are read one visible conversation at a time; they must not
// enumerate chat titles. The longest suffix is "_get_messages" (13 bytes), so a
// provider id above 51 would produce an MCP tool name the client rejects.
const LIST_SOURCES: [&str; 2] = ["local_index", "structured_db"];
const MAX_PROVIDER_LEN: usize = 51;

fn provider_id(provider: &str) -> Result<&str> {
    if !mcp_provider::identifier(provider)
        || provider == "."
        || provider == ".."
        || provider.len() > MAX_PROVIDER_LEN
    {
        bail!("无效的聊天来源 Provider");
    }
    Ok(provider)
}

fn payload(provider: &str, envelope: Value) -> Result<Value> {
    if envelope["ok"] != true || envelope["result"]["isError"] == true {
        if envelope["result"]["content"]
            .as_array()
            .is_some_and(|blocks| {
                blocks.iter().any(|b| {
                    b["text"]
                        .as_str()
                        .is_some_and(|t| t.contains("source_access_denied"))
                })
            })
        {
            if provider == "wechat" {
                bail!("微信数据源访问被 macOS 拒绝。Local Bridge 不应持有完全磁盘访问权限；请使用已授权的独立 WeChat Provider 读取服务，或改用已授权快照、已有索引与手动粘贴。当前 Provider 若由 Bridge 直接启动，macOS 仍可能把权限归属 Bridge；需由 Provider 独立运行后再验收。不要反复给开发版 Bridge 授权或重启微信。");
            }
            bail!("Provider 拒绝读取本机数据；请检查其权限与账号状态。未自动重试。");
        }
        if provider == "wechat" {
            bail!("微信 Provider 调用失败；未自动重试");
        }
        bail!("聊天来源 Provider 调用失败；未自动重试");
    }
    let result = &envelope["result"];
    let data = if result["structuredContent"].is_object() {
        result["structuredContent"].clone()
    } else {
        let blocks = result["content"]
            .as_array()
            .context("Provider 返回格式不支持")?;
        if blocks.len() != 1 || blocks[0]["type"] != "text" {
            bail!("Provider 返回格式不支持");
        }
        serde_json::from_str(blocks[0]["text"].as_str().context("Provider 返回为空")?)?
    };
    if !data.is_object() || data.get("error").is_some() {
        bail!("Provider 未完成读取；请检查原 Provider 状态");
    }
    Ok(data)
}
fn invoke(provider: &str, tool: &str, args: Value) -> Result<Value> {
    payload(
        provider,
        mcp_provider::call(provider, &format!("{provider}_{tool}"), &args.to_string())?,
    )
}
pub fn status(provider: &str) -> Result<Value> {
    let provider = provider_id(provider)?;
    let registered = mcp_provider::list()?["providers"]
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v["id"] == provider));
    if !registered {
        return Ok(json!({"provider":provider,"registered":false}));
    }
    let data = invoke(provider, "status", json!({}))?;
    Ok(status_view(provider, data))
}
fn status_view(provider: &str, data: Value) -> Value {
    json!({"provider":provider,"registered":true,"protocol_responding":true,
        "read_contract_supported":data["read_contract_version"] == 1,
        "local_index_read":data["capabilities"]["local_index_read"] == true,
        "structured_db_read":data["capabilities"]["structured_db_read"] == true,
        "live_read_available":data["capabilities"]["live_read_available"] == true,
        "live_read_verified":data["capabilities"]["live_read_verified"] == true,
        "web_page_read":data["capabilities"]["web_page_read"] == true,
        "platform":data["adapter"]["platform"],"blocked_reason":data["adapter"]["blocked_reason"],
        "send_enabled":false})
}
pub fn conversations(provider: &str, source: &str, cursor: Option<String>) -> Result<Value> {
    let provider = provider_id(provider)?;
    if !LIST_SOURCES.contains(&source) || cursor.as_ref().is_some_and(|v| v.len() > 4096) {
        bail!("此来源暂不支持会话选择");
    }
    let state = status(provider)?;
    if state["registered"] != true {
        bail!("该聊天来源 Provider 尚未注册；请先在设置中连接，或改用手动粘贴。未自动重试。");
    }
    source_supported(provider, &state, source)?;
    let data = invoke(
        provider,
        "list_chats",
        json!({"source":source,"limit":100,"cursor":cursor}),
    )?;
    if data["source"] != source
        || (source == "structured_db" && data["id_kind"] != "stable_conversation_id")
    {
        bail!("会话列表来源不匹配");
    }
    let rows = data["chats"].as_array().context("会话列表无效")?;
    if rows.len() > 100 {
        bail!("会话列表过长");
    }
    for row in rows {
        let id = row["chat_id"].as_str().context("会话身份缺失")?;
        validate_request(provider, id, source, 10)?;
    }
    Ok(data)
}
fn validate_request(provider: &str, chat_id: &str, source: &str, limit: u32) -> Result<()> {
    if provider_id(provider).is_err()
        || chat_id.trim().is_empty()
        || chat_id.chars().count() > 200
        || chat_id.contains('\0')
        || !SOURCES.contains(&source)
        || !(1..=20).contains(&limit)
    {
        bail!("请填写明确的会话显示名并选择 1–20 条片段");
    }
    Ok(())
}
fn source_supported(provider: &str, state: &Value, source: &str) -> Result<()> {
    if state["read_contract_supported"] != true {
        if provider == "wechat" {
            bail!("请连接支持读取来源标记的新版微信 Provider");
        }
        bail!("该 Provider 未声明读取合约版本 1；请升级 Provider 或改用手动粘贴。");
    }
    let available = match source {
        "local_index" => state["local_index_read"] == true,
        "desktop_ui" => state["live_read_available"] == true,
        "structured_db" => state["structured_db_read"] == true,
        "web_page" => state["web_page_read"] == true,
        _ => bail!("此来源暂不支持读取"),
    };
    if available {
        return Ok(());
    }
    match source {
        "local_index" => bail!("当前 Provider 未声明索引读取能力"),
        "desktop_ui" => {
            if provider == "wechat" {
                bail!("当前系统未开放实时读取；可改选已有索引或手动粘贴。未操作微信。");
            }
            bail!("当前 Provider 未开放实时读取；可改用手动粘贴。未操作聊天软件。");
        }
        "structured_db" => {
            if provider == "wechat" {
                bail!("结构化读取未配置；需要由用户先安装并绑定只读数据来源。未操作微信。");
            }
            bail!("结构化读取未配置；需要由用户先安装并绑定只读数据来源。未操作聊天软件。")
        }
        _ => bail!("当前 Provider 未声明网页会话读取能力；可改用手动粘贴。未访问浏览器。"),
    }
}
pub fn read(provider: &str, chat_id: &str, source: &str, limit: u32) -> Result<Value> {
    validate_request(provider, chat_id, source, limit)?;
    let state = status(provider)?;
    if state["registered"] != true {
        bail!("该聊天来源 Provider 尚未注册；请先在设置中连接，或改用手动粘贴。未自动重试。");
    }
    source_supported(provider, &state, source)?;
    let data = invoke(
        provider,
        "get_messages",
        json!({"chat_id":chat_id.trim(),"limit":limit,"source":source}),
    )?;
    normalize(provider, data, chat_id.trim(), source, limit)
}
fn normalize(
    provider: &str,
    data: Value,
    chat_id: &str,
    requested: &str,
    limit: u32,
) -> Result<Value> {
    if data["chat_id"] != chat_id {
        bail!("Provider 返回的会话不匹配，未采用片段");
    }
    let returned = data["source"].as_str().context("Provider 未返回读取来源")?;
    let cached_reported = returned == "local_index" && data["read_status"] == "cached_fallback";
    let cached_fallback = requested == "desktop_ui" && cached_reported;
    if returned != requested && !cached_fallback {
        bail!("Provider 返回的来源与本轮选择不匹配");
    }
    let structured = requested == "structured_db";
    let web = requested == "web_page";
    if structured
        && (data["id_kind"] != "stable_conversation_id"
            || !data["source_identity"]
                .as_str()
                .is_some_and(|id| !id.is_empty() && id.len() <= 512))
    {
        bail!("Provider 缺少结构化来源身份，未采用片段");
    }
    if returned == "desktop_ui" && data["live_read"] != true {
        bail!("Provider 未确认此次实时读取，未采用片段");
    }
    if web && data["live_read"] != true {
        bail!("Provider 未确认此次网页会话读取，未采用片段");
    }
    let rows = data["messages"]
        .as_array()
        .context("Provider 未返回消息列表")?;
    if rows.len() > limit as usize {
        bail!("Provider 返回超出所选范围");
    }
    if (returned == "desktop_ui" || web) && data["read_status"] == "cached_fallback" {
        bail!("Provider 返回缓存片段，不能作为实时读取，未采用片段");
    }
    let mut total = 0;
    let mut messages = Vec::new();
    let mut skipped_non_text = 0;
    for row in rows {
        if (structured || web) && row.get("chat_id").is_some() && row["chat_id"] != chat_id {
            bail!("Provider 消息包含其他会话");
        }
        let text = match row["text"].as_str() {
            Some(text) => text,
            None if (structured || web) && row["content_kind"] == "non_text" => {
                skipped_non_text += 1;
                continue;
            }
            None => bail!("Provider 消息格式不支持"),
        };
        total += text.len();
        if total > 32_000 || text.contains('\0') {
            bail!("片段过长，请减少读取条数");
        }
        messages.push(json!({
            "text":text,"sender":row["sender"],"sent_at":row["sent_at"],
            "source":row["source"],"imported_at":row["imported_at"],
            "message_id":if structured {&row["message_id"]} else {&Value::Null}
        }));
    }
    Ok(json!({
        "provider":provider,
        "chat_id":chat_id,
        "source":returned,
        "messages":messages,
        "complete":false,
        "id_kind":if structured {"stable_conversation_id"} else {"display_name"},
        "source_identity":if structured {&data["source_identity"]} else {&Value::Null},
        "coverage_complete":if structured {data["coverage_complete"].clone()} else if web {json!(false)} else {Value::Null},
        "has_more":if structured {&data["has_more"]} else {&Value::Null},
        "next_cursor":if structured {&data["next_cursor"]} else {&Value::Null},
        "skipped_non_text":skipped_non_text,
        "live_read":(returned == "desktop_ui" || returned == "web_page") && data["live_read"] == true,
        "session_visible_only":web,
        "cached_fallback":cached_reported
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_explicit_reads_are_valid() {
        assert!(validate_request("wechat", "示例", "local_index", 10).is_ok());
        assert!(validate_request("soul", "示例", "web_page", 10).is_ok());
        for (provider, name, source, limit) in [
            ("wechat", "", "local_index", 10),
            ("wechat", "x", "send", 1),
            ("wechat", "x", "desktop_ui", 21),
            ("bad provider!", "x", "web_page", 10),
            ("soul", "x", "unknown", 10),
        ] {
            assert!(validate_request(provider, name, source, limit).is_err());
        }
    }
    #[test]
    fn cached_fallback_is_never_presented_as_live() {
        let result = normalize(
            "wechat",
            json!({"chat_id":"test","source":"local_index","read_status":"cached_fallback",
            "messages":[{"text":"synthetic","source":"jsonl"}]}),
            "test",
            "local_index",
            10,
        )
        .unwrap();
        assert_eq!(result["live_read"], false);
        assert_eq!(result["cached_fallback"], true);
        assert_eq!(result["provider"], "wechat");
        assert_eq!(result["messages"][0]["source"], "jsonl");
        assert!(normalize(
            "wechat",
            json!({"chat_id":"other","source":"desktop_ui","messages":[]}),
            "test",
            "local_index",
            10
        )
        .is_err());
    }
    #[test]
    fn provider_cannot_exceed_requested_count() {
        assert!(normalize(
            "wechat",
            json!({"chat_id":"test","source":"local_index",
            "messages":[{"text":"one"},{"text":"two"}]}),
            "test",
            "local_index",
            1
        )
        .is_err());
    }
    #[test]
    fn structured_reader_rejects_wrong_source_or_identity() {
        let valid = json!({
            "chat_id":"wxid_a","source":"structured_db",
            "id_kind":"stable_conversation_id","source_identity":"sha256:synthetic",
            "coverage_complete":false,"messages":[
                {"chat_id":"wxid_a","text":"hello","sender":"example",
                 "source":"structured_db","message_id":"opaque-m1"},
                {"chat_id":"wxid_a","text":null,"content_kind":"non_text"}
            ]
        });
        let result = normalize("wechat", valid.clone(), "wxid_a", "structured_db", 2).unwrap();
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["id_kind"], "stable_conversation_id");
        assert_eq!(result["skipped_non_text"], 1);
        assert_eq!(result["live_read"], false);
        assert!(normalize("wechat", valid.clone(), "wxid_a", "local_index", 2).is_err());
        let mut changed = valid.clone();
        changed["source_identity"] = Value::Null;
        assert!(normalize("wechat", changed, "wxid_a", "structured_db", 2).is_err());
        changed = valid.clone();
        changed["messages"][0]["chat_id"] = json!("other");
        assert!(normalize("wechat", changed, "wxid_a", "structured_db", 2).is_err());
    }
    #[test]
    fn permission_denial_has_actionable_message() {
        let error = payload(
            "wechat",
            json!({"ok":false,"result":{"content":[{"type":"text","text":"source_access_denied"}]}}),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("Local Bridge 不应持有完全磁盘访问权限"));
        assert!(!error.to_string().contains("允许 Local Bridge Desktop Dev"));
        let generic = payload(
            "soul",
            json!({"ok":false,"result":{"content":[{"type":"text","text":"source_access_denied"}]}}),
        )
        .unwrap_err();
        assert!(!generic.to_string().contains("微信"));
    }
    #[test]
    fn mcp_error_is_not_an_empty_chat() {
        assert!(payload("wechat", json!({"ok":false})).is_err());
        assert!(payload(
            "wechat",
            json!({"ok":true,"result":{"structuredContent":{"error":"blocked"}}})
        )
        .is_err());
        assert_eq!(
            payload(
                "wechat",
                json!({"ok":true,"result":{"content":[{"type":"text","text":"{\"messages\":[]}"}]}})
            )
            .unwrap()["messages"],
            json!([])
        );
    }
    #[test]
    fn status_view_reports_provider_and_web_page_capability() {
        let view = status_view(
            "soul",
            json!({"read_contract_version":1,"capabilities":{"web_page_read":true},
                "adapter":{"platform":"soul-web"}}),
        );
        assert_eq!(view["provider"], "soul");
        assert_eq!(view["web_page_read"], true);
        assert_eq!(view["local_index_read"], false);
        assert_eq!(view["send_enabled"], false);
    }
    #[test]
    fn web_page_read_requires_declared_capability() {
        let capable = json!({"read_contract_supported":true,"web_page_read":true});
        assert!(source_supported("soul", &capable, "web_page").is_ok());
        assert!(
            source_supported("soul", &json!({"read_contract_supported":true}), "web_page").is_err()
        );
        assert!(source_supported(
            "soul",
            &json!({"read_contract_supported":false,"web_page_read":true}),
            "web_page"
        )
        .is_err());
    }
    #[test]
    fn web_page_read_marks_visible_only_and_skips_non_text() {
        let result = normalize(
            "soul",
            json!({"chat_id":"示例会话","source":"web_page","live_read":true,
                "messages":[{"text":"hello","sender":"peer","source":"cache"},
                            {"text":null,"content_kind":"non_text"}]}),
            "示例会话",
            "web_page",
            10,
        )
        .unwrap();
        assert_eq!(result["provider"], "soul");
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["skipped_non_text"], 1);
        assert_eq!(result["session_visible_only"], true);
        assert_eq!(result["coverage_complete"], false);
        assert_eq!(result["live_read"], true);
        assert!(normalize(
            "soul",
            json!({"chat_id":"示例会话","source":"web_page","messages":[]}),
            "示例会话",
            "web_page",
            10
        )
        .is_err());
        assert!(normalize(
            "soul",
            json!({"chat_id":"示例会话","source":"local_index","live_read":true,"messages":[]}),
            "示例会话",
            "web_page",
            10
        )
        .is_err());
        assert!(normalize(
            "soul",
            json!({"chat_id":"示例会话","source":"web_page","live_read":true,
                "read_status":"cached_fallback","messages":[]}),
            "示例会话",
            "web_page",
            10
        )
        .is_err());
        assert!(normalize(
            "soul",
            json!({"chat_id":"示例会话","source":"web_page","live_read":true,
                "messages":[{"chat_id":"other","text":"other session"}]}),
            "示例会话",
            "web_page",
            10
        )
        .is_err());
    }
    #[test]
    fn unknown_provider_or_source_is_rejected() {
        assert!(status("bad provider!").is_err());
        assert!(status(".").is_err());
        assert!(status("..").is_err());
        assert!(validate_request("../wechat", "x", "web_page", 10).is_err());
        assert!(validate_request("soul", "x", "web_page", 0).is_err());
        assert!(validate_request(&"a".repeat(51), "x", "web_page", 10).is_ok());
        assert!(validate_request(&"a".repeat(52), "x", "web_page", 10).is_err());
    }
    #[test]
    fn web_page_providers_cannot_enumerate_conversations() {
        assert!(conversations("soul", "web_page", None).is_err());
        assert!(conversations("bad provider!", "local_index", None).is_err());
    }
}
