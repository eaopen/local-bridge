//! Optional OS-user-only Tauri management window. No HTTP admin endpoint,
//! second task database, Web Agent grants, CloudFile file writes or tray daemon.
use local_bridge::{
    agent_health, agent_options, agent_preferences, diagnostics, ecosystem, mcp_provider,
    native_host_registration, task_center,
};
use serde_json::{json, Value};

#[tauri::command]
fn desktop_chat_selection() -> Result<Value, String> {
    agent_options::local_selection().map_err(|_| "没有兼容的已安装 Agent，请检查 Agent 设置".into())
}
#[tauri::command]
fn desktop_chat_run(
    session: tauri::State<'_, local_bridge::chat_copilot::Session>,
    input: local_bridge::chat_copilot::Input,
) -> Result<Value, String> {
    Ok(match session.run(input) {
        Ok(receipt) => receipt,
        Err(error) => json!({"status":"rejected","error":error.to_string()}),
    })
}
#[tauri::command]
fn desktop_chat_read(
    session: tauri::State<'_, local_bridge::chat_copilot::Session>,
    id: String,
) -> Result<Value, String> {
    session.read(&id).map_err(|e| e.to_string())
}
#[tauri::command]
fn desktop_chat_clear(
    session: tauri::State<'_, local_bridge::chat_copilot::Session>,
) -> Result<(), String> {
    session.clear().map_err(|e| e.to_string())
}

#[tauri::command]
fn desktop_overview() -> Value {
    let providers =
        mcp_provider::list().unwrap_or_else(|_| json!({"providers":[],"state":"unavailable"}));
    let host =
        native_host_registration::status().unwrap_or_else(|_| json!({"state":"unavailable"}));
    json!({
        "version":env!("CARGO_PKG_VERSION"),
        "ui_kind":"optional_local_desktop",
        "diagnostics":diagnostics::run(),
        "host":host,
        "mcp_providers":providers,
        "agents":agent_options::local_inventory(),
        "chrome_verified":false,
        "agent_login_verified":false,
        "cloudfile_write_capability":false
    })
}

#[tauri::command]
async fn desktop_agent_check(agent: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        agent_health::check(&agent).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "Agent 检查失败".to_string())?
}

#[tauri::command]
async fn desktop_tasks(include_cloudfile: bool) -> Result<Value, String> {
    // An optional CloudFile Provider starts an external MCP subprocess.
    // Keep the Tauri event loop responsive and never auto-query CloudFile.
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = task_center::list(include_cloudfile);
        result["local_cli_only"] = json!(false);
        result["trusted_desktop_only"] = json!(true);
        result
    })
    .await
    .map_err(|_| "任务列表读取失败".to_string())
}

#[tauri::command]
fn desktop_preferences_get() -> Result<Value, String> {
    agent_preferences::get_value().map_err(|error| error.to_string())
}

#[tauri::command]
fn desktop_preferences_save(preferences: Value) -> Result<Value, String> {
    // Same schema, private atomic file, OS lock, and validation as CLI.
    agent_preferences::save_value(preferences).map_err(|error| error.to_string())
}

#[tauri::command]
fn desktop_ecosystem_discover() -> Value {
    ecosystem::discover()
}

#[tauri::command]
async fn desktop_ecosystem_action(action: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || match action.as_str() {
        "pi_install" => ecosystem::install_pi().map_err(|e| e.to_string()),
        "codex_connect" => ecosystem::connect_codex().map_err(|e| e.to_string()),
        _ => Err("unsupported ecosystem action".to_owned()),
    })
    .await
    .map_err(|_| "ecosystem action cancelled".to_owned())?
}

#[tauri::command]
fn desktop_settings() -> Result<Value, String> {
    Ok(json!({
        "data_root": local_bridge::inbox::data_root().map_err(|e| e.to_string())?,
        "providers": mcp_provider::list().map_err(|e| e.to_string())?["providers"],
        "host": native_host_registration::status().map_err(|e| e.to_string())?
    }))
}

fn provider_args(id: &str) -> Result<Vec<String>, String> {
    match id {
        "wechat" => Ok(vec!["serve".into()]),
        "cloudfile" => Ok(vec!["--bridge-provider".into()]),
        _ => Err("当前设置页仅支持 CloudFile 和微信连接".into()),
    }
}

#[tauri::command]
fn desktop_provider_add(id: String, executable: String) -> Result<Value, String> {
    let args = provider_args(&id)?;
    mcp_provider::add(&id, &executable, &args).map_err(|e| e.to_string())
}

#[tauri::command]
fn desktop_provider_remove(id: String) -> Result<Value, String> {
    provider_args(&id)?;
    mcp_provider::remove(&id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn desktop_provider_tools(id: String) -> Result<Value, String> {
    provider_args(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        mcp_provider::tools(&id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "连接检查失败".to_string())?
}

#[tauri::command]
async fn desktop_chat_source_status() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| local_bridge::chat_source::status().map_err(|e| e.to_string()))
        .await.map_err(|_| "连接检查未完成".to_string())?
}
#[tauri::command]
async fn desktop_chat_source_conversations(source: String, cursor: Option<String>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || local_bridge::chat_source::conversations(&source, cursor).map_err(|e| e.to_string()))
        .await.map_err(|_| "会话列表加载失败".to_string())?
}
#[tauri::command]
async fn desktop_chat_source_read(chat_id: String, source: String, limit: u32) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || local_bridge::chat_source::read(&chat_id, &source, limit).map_err(|e| e.to_string()))
        .await.map_err(|_| "读取未完成；未自动重试".to_string())?
}

fn main() {
    tauri::Builder::default()
        .manage(local_bridge::chat_copilot::Session::default())
        .invoke_handler(tauri::generate_handler![
            desktop_chat_selection,
            desktop_chat_run,
            desktop_chat_read,
            desktop_chat_clear,
            desktop_overview,
            desktop_agent_check,
            desktop_tasks,
            desktop_preferences_get,
            desktop_preferences_save,
            desktop_settings,
            desktop_ecosystem_discover,
            desktop_ecosystem_action,
            desktop_provider_add,
            desktop_provider_remove,
            desktop_provider_tools,
            desktop_chat_source_status,
            desktop_chat_source_read,
            desktop_chat_source_conversations,
        ])
        .run(tauri::generate_context!())
        .expect("unable to start Local Bridge Desktop");
}

#[cfg(test)]
mod tests {
    use super::provider_args;

    #[test]
    fn settings_connections_only_use_known_fixed_entrypoints() {
        assert_eq!(provider_args("wechat").unwrap(), vec!["serve"]);
        assert_eq!(
            provider_args("cloudfile").unwrap(),
            vec!["--bridge-provider"]
        );
        for id in ["other", "../wechat", "wechat serve", ""] {
            assert!(provider_args(id).is_err());
        }
    }
}
