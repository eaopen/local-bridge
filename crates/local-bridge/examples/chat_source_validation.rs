//! Opt-in synthetic index -> real MCP -> Chat Copilot -> selected real Agent.
//! Use a temporary LOCAL_BRIDGE_DATA_DIR and WECHAT_PROVIDER_DB, never real history.
use local_bridge::{
    chat_copilot::{Input, Session},
    chat_source,
};
use serde_json::json;
use std::time::{Duration, Instant};
fn main() -> anyhow::Result<()> {
    let source = chat_source::read("BRIDGE_SYNTHETIC_FIXTURE", "local_index", 10)?;
    anyhow::ensure!(source["source"] == "local_index" && source["live_read"] == false);
    anyhow::ensure!(source["messages"].as_array().is_some_and(|m| m.len() == 1));
    #[cfg(target_os = "macos")]
    anyhow::ensure!(chat_source::read("BRIDGE_SYNTHETIC_FIXTURE", "desktop_ui", 1).is_err());
    let text = source["messages"][0]["text"].as_str().unwrap();
    let input: Input = serde_json::from_value(
        json!({"client_key":"source-fixture","mode":"reply","text":text,
      "intent":"安慰","voice":"自然简短","constraints":"不编造经历","feedback":"",
      "agent":"workbuddy","model":"glm-5.3-flash","think_mode":"low",
      "context_source":"wechat_local_index","thread_label":"BRIDGE_SYNTHETIC_FIXTURE"}),
    )?;
    let session = Session::default();
    let receipt = session.run(input)?;
    let id = receipt["task_id"].as_str().unwrap();
    let start = Instant::now();
    loop {
        let state = session.read(id)?;
        if state["status"] != "running" {
            anyhow::ensure!(state["status"] == "completed", "Agent did not complete");
            println!(
                "{}",
                json!({"mcp":"real","data":"synthetic_local_index","live_wechat":false,"agent":"workbuddy","status":state["status"],"elapsed_ms":start.elapsed().as_millis(),"result":state["result"]})
            );
            session.clear()?;
            break;
        }
        anyhow::ensure!(start.elapsed() < Duration::from_secs(190), "timeout");
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}
