//! Native-user initiated, bounded WeChat reads. No sends, automatic probes or AI calls.
use crate::mcp_provider;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

fn payload(envelope: Value) -> Result<Value> {
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
            bail!("微信数据源访问被 macOS 拒绝。Local Bridge 不应持有完全磁盘访问权限；请使用已授权的独立 WeChat Provider 读取服务，或改用已授权快照、已有索引与手动粘贴。当前 Provider 若由 Bridge 直接启动，macOS 仍可能把权限归属 Bridge；需由 Provider 独立运行后再验收。不要反复给开发版 Bridge 授权或重启微信。");
        }
        bail!("微信 Provider 调用失败；未自动重试");
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
fn invoke(tool: &str, args: Value) -> Result<Value> {
    payload(mcp_provider::call("wechat", tool, &args.to_string())?)
}
pub fn status() -> Result<Value> {
    let registered = mcp_provider::list()?["providers"]
        .as_array()
        .is_some_and(|items| items.iter().any(|v| v["id"] == "wechat"));
    if !registered {
        return Ok(json!({"registered":false}));
    }
    let data = invoke("wechat_status", json!({}))?;
    Ok(json!({"registered":true,"protocol_responding":true,
        "read_contract_supported":data["read_contract_version"] == 1,
        "local_index_read":data["capabilities"]["local_index_read"] == true,
        "structured_db_read":data["capabilities"]["structured_db_read"] == true,
        "live_read_available":data["capabilities"]["live_read_available"] == true,
        "live_read_verified":data["capabilities"]["live_read_verified"] == true,
        "platform":data["adapter"]["platform"],"blocked_reason":data["adapter"]["blocked_reason"],
        "send_enabled":false}))
}
pub fn conversations(source: &str, cursor: Option<String>) -> Result<Value> {
    if !["local_index", "structured_db"].contains(&source)
        || cursor.as_ref().is_some_and(|v| v.len() > 4096)
    {
        bail!("此来源暂不支持会话选择");
    }
    let data = invoke(
        "wechat_list_chats",
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
        validate_request(id, source, 10)?;
    }
    Ok(data)
}
fn validate_request(chat_id: &str, source: &str, limit: u32) -> Result<()> {
    if chat_id.trim().is_empty()
        || chat_id.chars().count() > 200
        || chat_id.contains('\0')
        || !["local_index", "desktop_ui", "structured_db"].contains(&source)
        || !(1..=20).contains(&limit)
    {
        bail!("请填写明确的会话显示名并选择 1–20 条片段");
    }
    Ok(())
}
pub fn read(chat_id: &str, source: &str, limit: u32) -> Result<Value> {
    validate_request(chat_id, source, limit)?;
    let state = status()?;
    if state["read_contract_supported"] != true {
        bail!("请连接支持读取来源标记的新版微信 Provider");
    }
    if source == "local_index" && state["local_index_read"] != true {
        bail!("当前 Provider 未声明索引读取能力");
    }
    if source == "desktop_ui" && state["live_read_available"] != true {
        bail!("当前系统未开放实时读取；可改选已有索引或手动粘贴。未操作微信。");
    }
    if source == "structured_db" && state["structured_db_read"] != true {
        bail!("结构化读取未配置；需要由用户先安装并绑定只读数据来源。未操作微信。");
    }
    let data = invoke(
        "wechat_get_messages",
        json!({"chat_id":chat_id.trim(),"limit":limit,"source":source}),
    )?;
    normalize(data, chat_id.trim(), source, limit)
}
fn normalize(data: Value, chat_id: &str, requested: &str, limit: u32) -> Result<Value> {
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
    let rows = data["messages"]
        .as_array()
        .context("Provider 未返回消息列表")?;
    if rows.len() > limit as usize {
        bail!("Provider 返回超出所选范围");
    }
    let mut total = 0;
    let mut messages = Vec::new();
    let mut skipped_non_text = 0;
    for row in rows {
        if structured && row["chat_id"] != chat_id {
            bail!("Provider 消息包含其他会话");
        }
        let text = match row["text"].as_str() {
            Some(text) => text,
            None if structured && row["content_kind"] == "non_text" => {
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
        "chat_id":chat_id,
        "source":returned,
        "messages":messages,
        "complete":false,
        "id_kind":if structured {"stable_conversation_id"} else {"display_name"},
        "source_identity":if structured {&data["source_identity"]} else {&Value::Null},
        "coverage_complete":if structured {&data["coverage_complete"]} else {&Value::Null},
        "has_more":if structured {&data["has_more"]} else {&Value::Null},
        "next_cursor":if structured {&data["next_cursor"]} else {&Value::Null},
        "skipped_non_text":skipped_non_text,
        "live_read":returned == "desktop_ui" && data["live_read"] == true,
        "cached_fallback":cached_reported
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_bounded_explicit_reads_are_valid() {
        assert!(validate_request("示例", "local_index", 10).is_ok());
        for (name, source, limit) in [
            ("", "local_index", 10),
            ("x", "send", 1),
            ("x", "desktop_ui", 21),
        ] {
            assert!(validate_request(name, source, limit).is_err());
        }
    }
    #[test]
    fn cached_fallback_is_never_presented_as_live() {
        let result = normalize(
            json!({"chat_id":"test","source":"local_index","read_status":"cached_fallback",
            "messages":[{"text":"synthetic","source":"jsonl"}]}),
            "test",
            "local_index",
            10,
        )
        .unwrap();
        assert_eq!(result["live_read"], false);
        assert_eq!(result["cached_fallback"], true);
        assert_eq!(result["messages"][0]["source"], "jsonl");
        assert!(normalize(
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
        let result = normalize(valid.clone(), "wxid_a", "structured_db", 2).unwrap();
        assert_eq!(result["messages"].as_array().unwrap().len(), 1);
        assert_eq!(result["id_kind"], "stable_conversation_id");
        assert_eq!(result["skipped_non_text"], 1);
        assert_eq!(result["live_read"], false);
        assert!(normalize(valid.clone(), "wxid_a", "local_index", 2).is_err());
        let mut changed = valid.clone();
        changed["source_identity"] = Value::Null;
        assert!(normalize(changed, "wxid_a", "structured_db", 2).is_err());
        changed = valid.clone();
        changed["messages"][0]["chat_id"] = json!("other");
        assert!(normalize(changed, "wxid_a", "structured_db", 2).is_err());
    }
    #[test]
    fn permission_denial_has_actionable_message() {
        let error = payload(json!({"ok":false,"result":{"content":[{"type":"text","text":"source_access_denied"}]}})).unwrap_err();
        assert!(error
            .to_string()
            .contains("Local Bridge 不应持有完全磁盘访问权限"));
        assert!(!error.to_string().contains("允许 Local Bridge Desktop Dev"));
    }
    #[test]
    fn mcp_error_is_not_an_empty_chat() {
        assert!(payload(json!({"ok":false})).is_err());
        assert!(
            payload(json!({"ok":true,"result":{"structuredContent":{"error":"blocked"}}})).is_err()
        );
        assert_eq!(
            payload(
                json!({"ok":true,"result":{"content":[{"type":"text","text":"{\"messages\":[]}"}]}})
            )
            .unwrap()["messages"],
            json!([])
        );
    }
}
