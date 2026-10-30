//! Opt-in REAL model validation with invented conversations only. Never in CI.
//! cargo run -p local-bridge --example chat_copilot_validation -- codex
use local_bridge::chat_copilot::{Input, Session};
use serde_json::json;
use std::time::{Duration, Instant};
fn main() -> anyhow::Result<()> {
    let agent = std::env::args().nth(1).unwrap_or_else(|| "codex".into());
    let model = std::env::args().nth(2).unwrap_or_else(|| "auto".into());
    let think = std::env::args().nth(3).unwrap_or_else(|| "auto".into());
    let fixture = std::env::args().nth(4);
    for (name, mode, text, intent) in [
        (
            "friend",
            "reply",
            "朋友：明天又要出差，有点累。",
            "安慰，不用约见",
        ),
        (
            "work",
            "reply",
            "同事：今晚能帮我把报告做完吗？我今晚有家事，不能加班。",
            "礼貌拒绝",
        ),
        (
            "repair",
            "reflect",
            "我刚才语气重了，对方说先冷静一下。我想道歉，但不想逼对方马上回应。",
            "关系修复",
        ),
        (
            "silence",
            "understand",
            "对方两天没回复。我不知道原因。",
            "区分事实与推测",
        ),
    ] {
        if fixture.as_deref().is_some_and(|selected| selected != name) {
            continue;
        }
        let session = Session::default();
        let input: Input = serde_json::from_value(
            json!({"client_key":name,"mode":mode,"text":text,"intent":intent,
            "voice":"自然、简短、不油腻","constraints":"不编造经历和承诺","feedback":"",
            "agent":agent,"model":model,"think_mode":think}),
        )?;
        let receipt = session.run(input)?;
        let id = receipt["task_id"].as_str().unwrap();
        let start = Instant::now();
        loop {
            let result = session.read(id)?;
            if result["status"] != "running" {
                println!(
                    "{}",
                    json!({"fixture":name,"real_agent":agent,"elapsed_ms":start.elapsed().as_millis(),"receipt":result})
                );
                session.clear()?;
                if result["status"] != "completed" {
                    anyhow::bail!("real Agent validation failed");
                }
                break;
            }
            if start.elapsed() > Duration::from_secs(195) {
                anyhow::bail!("real validation timeout");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    Ok(())
}
