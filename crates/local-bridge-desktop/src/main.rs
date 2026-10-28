//! Optional OS-user-only Tauri management window. No HTTP admin endpoint,
//! second task database, Web Agent grants, CloudFile file writes or tray daemon.
use local_bridge::{
    agent_options, agent_preferences, diagnostics, mcp_provider, native_host_registration,
    task_center,
};
use serde_json::{json, Value};

#[tauri::command]
fn desktop_overview() -> Value {
    let providers = mcp_provider::list()
        .unwrap_or_else(|_| json!({"providers":[],"state":"unavailable"}));
    let host = native_host_registration::status()
        .unwrap_or_else(|_| json!({"state":"unavailable"}));
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            desktop_overview,
            desktop_tasks,
            desktop_preferences_get,
            desktop_preferences_save,
        ])
        .run(tauri::generate_context!())
        .expect("unable to start Local Bridge Desktop");
}
