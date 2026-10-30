//! Explicit OS-user availability checks. Discovery never invokes a model.
//! Only a fixed synthetic prompt is sent; no credentials or raw output persist.
use crate::{agent, agent_options, agent_preferences};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, TryLockError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

static CHECK_LOCK: Mutex<()> = Mutex::new(());
const MARKER: &str = "LOCAL_BRIDGE_AGENT_READY";
const PROMPT: &str = "Availability check. Do not use tools or read files. Reply with exactly LOCAL_BRIDGE_AGENT_READY and nothing else.";

pub(crate) struct Workspace(pub(crate) PathBuf);
impl Workspace {
    pub(crate) fn new() -> Result<Self> {
        let root = std::env::temp_dir().join(format!(
            "local-bridge-agent-check-{}-{}",
            std::process::id(),
            now()
        ));
        let mut builder = fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&root)?;
        Ok(Self(root))
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn codex_auth() -> Option<bool> {
    #[cfg(target_os = "macos")]
    let candidates = ["codex", "/opt/homebrew/bin/codex", "/usr/local/bin/codex"];
    #[cfg(not(target_os = "macos"))]
    let candidates = ["codex"];
    for program in candidates {
        let mut command = Command::new(program);
        #[cfg(target_os = "macos")]
        {
            let raw = std::env::var_os("PATH").unwrap_or_default();
            let dirs = std::env::split_paths(&raw).chain(
                ["/opt/homebrew/bin", "/usr/local/bin"]
                    .into_iter()
                    .map(PathBuf::from),
            );
            if let Ok(paths) = std::env::join_paths(dirs) {
                command.env("PATH", paths);
            }
        }
        command
            .args(["login", "status"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let Ok(mut child) = command.spawn() else {
            continue;
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Some(status.success()),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
        }
    }
    None
}

fn confirmed(output: &Value) -> bool {
    output["status"] == "completed"
        && output["result"]
            .as_str()
            .is_some_and(|text| text.trim() == MARKER)
}

#[cfg(target_os = "macos")]
fn workbuddy_check(root: &Path, model: &str, think: &str) -> Result<Value> {
    use crate::workbuddy_web;
    use std::io::Write;
    workbuddy_web::assets(root)?;
    let mut child = Command::new(workbuddy_web::node_path()?)
        .args(["--experimental-strip-types"])
        .arg(root.join("worker.mjs"))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("WorkBuddy worker unavailable")?;
    let sent = child
        .stdin
        .take()
        .context("WorkBuddy input missing")?
        .write_all(&serde_json::to_vec(
            &json!({"dir":root,"prompt":PROMPT,"model":model,"think_mode":think}),
        )?);
    if sent.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        bail!("WorkBuddy input unavailable");
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("WorkBuddy check timed out");
            }
        }
    }
    let path = root.join("events.jsonl");
    if fs::metadata(&path)?.len() > 512_000 {
        bail!("WorkBuddy event limit");
    }
    let rows: Vec<Value> = fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let complete = rows
        .last()
        .is_some_and(|row| row["type"] == "task.completed");
    let answer: String = rows
        .iter()
        .filter(|row| row["type"] == "message.delta")
        .filter_map(|row| row["text"].as_str())
        .collect();
    if !complete {
        bail!("WorkBuddy did not confirm completion");
    }
    Ok(json!({"status":"completed","result":answer}))
}
#[cfg(not(target_os = "macos"))]
fn workbuddy_check(_root: &Path, _model: &str, _think: &str) -> Result<Value> {
    bail!("WorkBuddy adapter unsupported on this platform")
}

/// Invoked only from a trusted local CLI or desktop click, never discovery/Web.
/// Uses the same adapters and current defaults; no fallback to another Agent.
pub fn check(name: &str) -> Result<Value> {
    let inventory = agent_options::local_inventory();
    let item = inventory
        .as_array()
        .and_then(|items| items.iter().find(|item| item["agent"] == name))
        .context("unknown Agent provider")?;
    let _guard = match CHECK_LOCK.try_lock() {
        Ok(lock) => lock,
        Err(TryLockError::WouldBlock) => bail!("another Agent availability check is active"),
        Err(TryLockError::Poisoned(_)) => bail!("Agent check lock unavailable; restart Bridge"),
    };
    let start = Instant::now();
    let mut result = json!({"agent":name,"installed":item["installed"],"authenticated":null,
        "api_ready":null,"run_completed":false,"ready":false,"state":"not_installed",
        "reason":"未发现可用客户端，请先安装。","checked_at_unix_ms":now(),"duration_ms":0});
    if item["installed"] != true {
        return Ok(result);
    }
    if item["run_supported"] != true {
        result["state"] = json!("unsupported_platform");
        result["reason"] = json!("当前平台的 Bridge 适配器尚不支持运行。");
        return Ok(result);
    }
    let preferences = agent_preferences::load()?;
    let (model, think) = if preferences.agent == name || preferences.agent == "auto" {
        (preferences.model.as_str(), preferences.think_mode.as_str())
    } else {
        ("auto", "auto")
    };
    result["model"] = json!(model);
    result["think_mode"] = json!(think);
    if !agent_options::options_supported(name, model, think) {
        result["state"] = json!("unsupported_options");
        result["reason"] =
            json!("已保存的模型或思考参数不受此 Agent 适配器支持，请先调整 Agent 设置。");
        return Ok(result);
    }
    if name == "codex" {
        let authenticated = codex_auth();
        result["authenticated"] = json!(authenticated);
        if authenticated == Some(false) {
            result["state"] = json!("not_authenticated");
            result["reason"] = json!("Codex CLI 登录检查未通过，请在原 Codex CLI 完成登录。");
            result["duration_ms"] = json!(start.elapsed().as_millis() as u64);
            return Ok(result);
        }
    }
    let workspace = Workspace::new()?;
    let outcome = match name {
        "codex" => agent::run_codex_in(PROMPT, model, think, Some(&workspace.0)),
        "pi" => agent::run_pi_check(PROMPT, model, think, &workspace.0),
        "workbuddy" => workbuddy_check(&workspace.0, model, think),
        _ => unreachable!(),
    };
    match outcome {
        Ok(output) if confirmed(&output) => {
            result["authenticated"] = json!(true);
            result["api_ready"] = json!(true);
            result["run_completed"] = json!(true);
            result["ready"] = json!(true);
            result["state"] = json!("ready");
            result["reason"] = json!("固定短任务已完成，回复匹配，当前调用可用。");
        }
        Ok(_) => {
            result["api_ready"] = json!(true);
            result["state"] = json!("response_mismatch");
            result["reason"] = json!("接口返回了结果，但未匹配验证文本，可用性尚未确认。");
        }
        Err(_) => {
            result["api_ready"] = json!(false);
            result["state"] = json!("check_failed");
            result["reason"]=json!(match name {
                "pi"=>"Pi 未完成固定短任务。请检查 Pi 原有的当前模型、认证、额度和网络；未自动切换模型。",
                "workbuddy"=>"WorkBuddy 原账号连接或短任务未完成。请检查桌面程序已运行、已登录且本机连接获准。",
                _=>"Codex 短任务未完成。请检查当前模型、账号权限、额度和网络。",
            });
        }
    }
    result["checked_at_unix_ms"] = json!(now());
    result["duration_ms"] = json!(start.elapsed().as_millis() as u64);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_and_exact_response_are_both_required() {
        assert!(confirmed(&json!({"status":"completed","result":MARKER})));
        assert!(!confirmed(&json!({"status":"running","result":MARKER})));
        assert!(!confirmed(
            &json!({"status":"completed","result":"not the marker"})
        ));
        assert!(!confirmed(&json!({"status":"accepted","result":MARKER})));
    }
    #[test]
    fn unknown_provider_fails_without_starting_processes() {
        assert!(check("../unknown").is_err());
    }
}
