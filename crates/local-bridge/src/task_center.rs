//! CLI-only read-only task center. Each module remains the factual task owner.
//! No task database, Web/AI tool exposure, file operations or origin grants.
use crate::{agent_task, inbox, mcp_provider, workbuddy_web};
use anyhow::{bail, Result};
use serde_json::{json, Value};

const CLOUDFILE_TOOL: &str = "cloudfile_task_summaries";
const MAX_CLOUDFILE: usize = 80;

fn source(name: &str, state: &str, count: usize, truncated: bool) -> Value {
    json!({"source":name,"state":state,"count":count,"truncated":truncated})
}

fn inbox_item(item: &Value) -> Value {
    let title = item["title"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect::<String>();
    json!({
        "task_id":item["id"],
        "owner_module":"inbox",
        "kind":"inbox",
        "provider":item["source"],
        "title":title,
        "status":"needs_action",
        "stage":"awaiting_user_review",
        "created_at_unix_ms":item["created_at_unix_ms"],
        "available_actions":[],
    })
}

/// Only copy fields explicitly guaranteed by the Agent2 P1 Provider contract.
/// A registered MCP executable is still an external input, not a trusted
/// business-state authority. File names/paths/tickets must not pass through.
fn cloudfile_rows(response: &Value) -> Result<(Vec<Value>, bool)> {
    if response["ok"] != true || response["result"]["isError"] != false {
        bail!("CloudFile Provider did not return successful read-only summaries");
    }
    let body = response["result"]["content"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing CloudFile Provider text result"))?;
    if body.len() > 128 * 1024 {
        bail!("CloudFile task summary exceeds bound");
    }
    let value: Value = serde_json::from_str(body)?;
    if value["schema_version"] != 1
        || value["provider_id"] != "cloudfile"
        || value["storage_scope"] != "agent2_shared"
        || value["data_owner"] != "cloudfile-agent2"
        || value["read_only"] != true
    {
        bail!("incompatible CloudFile task summary contract");
    }
    let items = value["items"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("invalid CloudFile tasks list"))?;
    let total = value["total"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("CloudFile task count missing"))?;
    let truncated = value["truncated"]
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("CloudFile truncation state missing"))?;
    if items.len() > MAX_CLOUDFILE
        || total < items.len() as u64
        || (truncated && total <= items.len() as u64)
        || (!truncated && total != items.len() as u64)
    {
        bail!("CloudFile task summary is inconsistent");
    }
    let mut rows = Vec::new();
    for item in items {
        let id = item["task_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("CloudFile task ID missing"))?;
        if id.len() != 64
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            bail!("invalid CloudFile task ID");
        }
        let mode = item["mode"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("CloudFile mode missing"))?;
        if !matches!(
            mode,
            "local-view" | "local-edit" | "optimistic-edit" | "exclusive-edit"
        ) {
            bail!("unknown CloudFile task mode");
        }
        let status = item["status"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("CloudFile task status missing"))?;
        if !matches!(status, "running" | "needs_action" | "completed" | "unknown") {
            bail!("invalid CloudFile task status");
        }
        let stage = item["stage"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("CloudFile stage missing"))?;
        if !matches!(
            stage,
            "original_publish_unconfirmed"
                | "remote_conflict"
                | "checkin_confirmed"
                | "local_view_available"
                | "local_edit_session"
        ) {
            bail!("invalid CloudFile stage");
        }
        rows.push(json!({
            "task_id":id,
            "owner_module":"cloudfile",
            "kind":"file",
            "provider":"cloudfile",
            "title":"CloudFile file task",
            "mode":mode,
            "status":status,
            "stage":stage,
            "created_at_unix_ms":null,
            "available_actions":[],
        }));
    }
    Ok((rows, truncated))
}

fn cloudfile() -> Result<(Vec<Value>, bool)> {
    // No Provider process is launched for the ordinary task listing.
    let providers = mcp_provider::list()?;
    if !providers["providers"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["id"] == "cloudfile"))
    {
        bail!("CloudFile Provider is not configured");
    }
    cloudfile_rows(&mcp_provider::call("cloudfile", CLOUDFILE_TOOL, "{}")?)
}

/// A local user explicitly runs this command. Do NOT expose it through the
/// Bridge MCP server, Web V2 status tools, or Chrome content scripts.
pub fn list(include_cloudfile: bool) -> Value {
    let mut items = Vec::new();
    let mut sources = Vec::new();
    match agent_task::summaries() {
        Ok(rows) => {
            sources.push(source("codex_pi", "ok", rows.len(), false));
            items.extend(rows);
        }
        Err(_) => sources.push(source("codex_pi", "unavailable", 0, false)),
    }
    match workbuddy_web::summaries() {
        Ok(rows) => {
            sources.push(source("workbuddy", "ok", rows.len(), false));
            items.extend(rows);
        }
        Err(_) => sources.push(source("workbuddy", "unavailable", 0, false)),
    }
    match inbox::list() {
        Ok(result) => {
            if let Some(rows) = result["items"].as_array() {
                sources.push(source("inbox", "ok", rows.len(), rows.len() == 100));
                items.extend(rows.iter().map(inbox_item));
            } else {
                sources.push(source("inbox", "unavailable", 0, false));
            }
        }
        Err(_) => sources.push(source("inbox", "unavailable", 0, false)),
    }
    if include_cloudfile {
        match cloudfile() {
            Ok((rows, truncated)) => {
                sources.push(source("cloudfile", "ok", rows.len(), truncated));
                items.extend(rows);
            }
            Err(_) => sources.push(source(
                "cloudfile",
                "not_configured_or_unavailable",
                0,
                false,
            )),
        }
    } else {
        sources.push(source("cloudfile", "not_requested", 0, false));
    }
    // Timestamp-free file tasks come after timestamped Agent/Inbox records;
    // this ordering is purely presentational and implies no new file state.
    items.sort_by(|a, b| {
        b["created_at_unix_ms"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["created_at_unix_ms"].as_u64().unwrap_or(0))
    });
    let complete = sources.iter().all(|src| {
        matches!(src["state"].as_str(), Some("ok" | "not_requested")) && src["truncated"] == false
    });
    json!({
        "schema_version":1,
        "read_only":true,
        "local_cli_only":true,
        "complete":complete,
        "sources":sources,
        "count":items.len(),
        "items":items
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_projection_drops_unexpected_sensitive_provider_fields() {
        let id = "a".repeat(64);
        let file = json!({
            "schema_version":1,"provider_id":"cloudfile",
            "storage_scope":"agent2_shared","data_owner":"cloudfile-agent2",
            "read_only":true,"total":1,"truncated":false,
            "items":[{
                "task_id":id,"mode":"exclusive-edit","status":"unknown",
                "stage":"original_publish_unconfirmed",
                "file_name":"private.dwg","server":"https://internal.test",
                "ticket":"PRIVATE-TICKET","remote_path":"/secrets/file"
            }]
        });
        let output = json!({
            "ok":true,"result":{
                "isError":false,"content":[{"type":"text","text":file.to_string()}]
            }
        });
        let (rows, truncated) = cloudfile_rows(&output).unwrap();
        assert!(!truncated);
        assert_eq!(rows[0]["status"], "unknown");
        assert_eq!(rows[0]["available_actions"], json!([]));
        for value in [
            "private.dwg",
            "internal.test",
            "PRIVATE-TICKET",
            "secrets/file",
        ] {
            assert!(!rows[0].to_string().contains(value));
        }
    }

    #[test]
    fn provider_contract_failure_does_not_become_empty_completed_list() {
        let file = json!({
            "schema_version":1,"provider_id":"cloudfile",
            "storage_scope":"unexpected","data_owner":"cloudfile-agent2",
            "read_only":true,"total":0,"truncated":false,"items":[]
        });
        assert!(cloudfile_rows(&json!({
            "ok":true,"result":{"isError":false,
                "content":[{"type":"text","text":file.to_string()}]}
        }))
        .is_err());
    }

    #[test]
    fn inbox_item_means_review_not_file_or_ai_execution() {
        let item = json!({
            "id":"ctx-0123456789abcdef","title":"Test document",
            "created_at_unix_ms":123,"source":"browser-extension",
            "selection":"private text not for task list"
        });
        let row = inbox_item(&item);
        assert_eq!(row["kind"], "inbox");
        assert_eq!(row["status"], "needs_action");
        assert!(!row.to_string().contains("private text"));
    }
}
