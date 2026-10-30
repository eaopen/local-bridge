use serde_json::json;
fn main() -> anyhow::Result<()> {
    let session = local_bridge::chat_copilot::Session::default();
    let input = serde_json::from_value(
        json!({"client_key":"structured-synthetic-proof","mode":"reply","text":"同事：明天上午十点一起确认测试结果，可以吗？","intent":"确认时间","voice":"自然简短","constraints":"","feedback":"","agent":"workbuddy","model":"glm-5.3-flash","think_mode":"low","context_source":"wechat_structured_db","thread_label":"synthetic"}),
    )?;
    session.run(input)?;
    for _ in 0..90 {
        let result = session.read("structured-synthetic-proof")?;
        if result["status"] == "completed" {
            anyhow::ensure!(result["result"].as_str().is_some_and(|s| !s.is_empty()));
            println!("Structured source -> WorkBuddy completed (synthetic content)");
            return Ok(());
        }
        if result["status"] == "failed" {
            anyhow::bail!("synthetic validation failed")
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    anyhow::bail!("validation timeout")
}
