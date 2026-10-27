//! User-owned local MCP stdio connectors. Independent providers (including
//! WeChat Provider) remain responsible for their authentication, message data,
//! side effects and receipts. The Bridge never parses vendor-specific storage.
//!
//! V1 is explicitly invoked from the local CLI; registered tools are NOT
//! automatically exposed to browser pages or to this Bridge's MCP server.
use crate::inbox;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_CONFIG: u64 = 16 * 1024;
const MAX_JSON_INPUT: usize = 64 * 1024;
const MAX_MCP_LINE: usize = 1024 * 1024;
const MCP_TIMEOUT: Duration = Duration::from_secs(90);
const PROTOCOL: &str = "2025-06-18";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderConfig {
    version: u32,
    id: String,
    executable: PathBuf,
    args: Vec<String>,
}

fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}

fn providers_dir() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("mcp-providers"))
}

fn path_for(dir: &Path, id: &str) -> Result<PathBuf> {
    if !identifier(id) || id == "." || id == ".." {
        bail!("invalid provider identifier");
    }
    Ok(dir.join(format!("{id}.json")))
}

fn private_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).context("cannot create local provider directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn validate_config(config: &ProviderConfig) -> Result<()> {
    if config.version != 1 || !identifier(&config.id) {
        bail!("invalid local provider configuration");
    }
    if !config.executable.is_absolute() || !config.executable.is_file() {
        bail!("provider executable must be an installed absolute local file");
    }
    if config.args.len() > 8
        || config
            .args
            .iter()
            .any(|x| x.len() > 256 || x.contains('\0') || x.contains('\n'))
    {
        bail!("invalid fixed provider arguments");
    }
    Ok(())
}

fn write_to(dir: &Path, id: &str, executable: &str, args: &[String]) -> Result<Value> {
    private_dir(dir)?;
    let target = PathBuf::from(executable);
    if !target.is_absolute() || !target.is_file() {
        bail!("provider executable must be an existing absolute path");
    }
    let config = ProviderConfig {
        version: 1,
        id: id.to_string(),
        executable: target.canonicalize().context("cannot resolve provider executable")?,
        args: args.to_vec(),
    };
    validate_config(&config)?;
    let path = path_for(dir, id)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .context("provider already registered or cannot be created; remove it first to change")?;
    if let Err(error) = serde_json::to_writer(&mut file, &config) {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(error).context("cannot save local provider");
    }
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(json!({"id":id,"registered":true,"state":"configured_not_verified"}))
}

pub fn add(id: &str, executable: &str, args: &[String]) -> Result<Value> {
    write_to(&providers_dir()?, id, executable, args)
}

fn load_from(dir: &Path, id: &str) -> Result<ProviderConfig> {
    let path = path_for(dir, id)?;
    let metadata = fs::metadata(&path).context("provider not registered")?;
    if !metadata.is_file() || metadata.len() > MAX_CONFIG {
        bail!("invalid provider configuration file");
    }
    let config: ProviderConfig =
        serde_json::from_slice(&fs::read(&path)?).context("invalid provider configuration JSON")?;
    validate_config(&config)?;
    if config.id != id {
        bail!("provider ID mismatch");
    }
    Ok(config)
}

fn load(id: &str) -> Result<ProviderConfig> {
    load_from(&providers_dir()?, id)
}

pub fn list() -> Result<Value> {
    let dir = providers_dir()?;
    if !dir.exists() {
        return Ok(json!({"providers":[]}));
    }
    let mut providers = Vec::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy().to_string();
        if let Some(id) = name.strip_suffix(".json") {
            if identifier(id) && load_from(&providers_dir()?, id).is_ok() {
                providers.push(json!({"id":id,"state":"configured_not_verified"}));
            }
        }
    }
    providers.sort_by(|a,b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(json!({"providers":providers}))
}

pub fn remove(id: &str) -> Result<Value> {
    let dir = providers_dir()?;
    fs::remove_file(path_for(&dir, id)?).context("provider not registered")?;
    Ok(json!({"id":id,"removed":true}))
}

fn parse_tool_name(name: &str) -> Result<()> {
    if !identifier(name) || name == "." || name == ".." {
        bail!("invalid MCP tool name");
    }
    Ok(())
}

fn read_json_input(input: &str) -> Result<Value> {
    let contents = if input == "-" {
        let mut bytes = Vec::new();
        io::stdin().take((MAX_JSON_INPUT + 1) as u64).read_to_end(&mut bytes)?;
        String::from_utf8(bytes).context("MCP arguments must be UTF-8 JSON")?
    } else {
        input.to_owned()
    };
    if contents.len() > MAX_JSON_INPUT {
        bail!("MCP arguments exceed the limit");
    }
    let value: Value = serde_json::from_str(&contents).context("MCP arguments must be JSON")?;
    if !value.is_object() {
        bail!("MCP arguments must be a JSON object");
    }
    Ok(value)
}

fn parse_response(result: Value, id: u32) -> Result<Value> {
    if result["id"] != id || result["jsonrpc"] != "2.0" {
        bail!("invalid MCP provider response ID or version");
    }
    if let Some(error) = result.get("error") {
        bail!("MCP provider returned an RPC error: {error}");
    }
    result
        .get("result")
        .cloned()
        .context("MCP provider response is missing result")
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Result<Value>>,
    reader: Option<JoinHandle<()>>,
}

impl Session {
    fn new(config: &ProviderConfig) -> Result<Self> {
        let mut child = Command::new(&config.executable)
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("cannot start user-registered MCP provider")?;
        let stdin = child.stdin.take().context("MCP stdin not available")?;
        let stdout = child.stdout.take().context("MCP stdout not available")?;
        let (tx, messages) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                let n = match (&mut reader)
                    .take((MAX_MCP_LINE + 1) as u64)
                    .read_until(b'\n', &mut bytes)
                {
                    Ok(n) => n,
                    Err(_) => {
                        let _ = tx.send(Err(anyhow::anyhow!("MCP provider output read failed")));
                        break;
                    }
                };
                if n == 0 {
                    let _ = tx.send(Err(anyhow::anyhow!("MCP provider closed the connection")));
                    break;
                }
                if n > MAX_MCP_LINE {
                    let _ = tx.send(Err(anyhow::anyhow!("MCP provider output exceeded limit")));
                    break;
                }
                let parsed = serde_json::from_slice(&bytes)
                    .context("MCP provider wrote invalid JSON to stdout");
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            messages,
            reader: Some(reader),
        })
    }

    fn notify(&mut self, method: &str) -> Result<()> {
        self.send(json!({"jsonrpc":"2.0","method":method}))
    }

    fn send(&mut self, value: Value) -> Result<()> {
        serde_json::to_writer(&mut self.stdin, &value)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush().context("cannot write to MCP provider")
    }

    fn request(&mut self, id: u32, method: &str, params: Value) -> Result<Value> {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let received = match self.messages.recv_timeout(MCP_TIMEOUT) {
                Ok(message) => message?,
                Err(RecvTimeoutError::Timeout) => bail!("MCP provider response timed out"),
                Err(RecvTimeoutError::Disconnected) => bail!("MCP provider disconnected"),
            };
            // Notifications may appear between MCP responses. Never mistake a
            // notification or another response for this request's completion.
            if received.get("id").is_some_and(|value| value == id) {
                return parse_response(received, id);
            }
        }
    }

    fn initialize(config: &ProviderConfig) -> Result<Self> {
        let mut session = Self::new(config)?;
        let ready = session.request(
            1,
            "initialize",
            json!({
                "protocolVersion":PROTOCOL,
                "capabilities":{},
                "clientInfo":{"name":"local-bridge","version":env!("CARGO_PKG_VERSION")}
            }),
        )?;
        if ready["protocolVersion"].as_str().is_none() {
            bail!("provider did not negotiate an MCP protocol version");
        }
        session.notify("notifications/initialized")?;
        Ok(session)
    }

    fn tools(&mut self) -> Result<Vec<Value>> {
        let response = self.request(2, "tools/list", json!({}))?;
        let tools = response["tools"]
            .as_array()
            .context("provider returned no tools list")?;
        if tools.len() > 128 {
            bail!("provider tool list too large");
        }
        Ok(tools.to_vec())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn exported_names(tools: &[Value]) -> Vec<&str> {
    tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect()
}

pub fn tools(id: &str) -> Result<Value> {
    let config = load(id)?;
    let mut session = Session::initialize(&config)?;
    let available = session.tools()?;
    Ok(json!({
        "provider":id,
        "transport":"stdio",
        "state":"mcp_responding_not_app_login_verified",
        "tools":exported_names(&available)
    }))
}

/// Explicit foreground CLI user action only. This includes provider-specific
/// sends; their own statuses and idempotency stay with that provider.
pub fn call(id: &str, tool: &str, json_input: &str) -> Result<Value> {
    parse_tool_name(tool)?;
    let args = read_json_input(json_input)?;
    let config = load(id)?;
    let mut session = Session::initialize(&config)?;
    let discovered = session.tools()?;
    if !exported_names(&discovered).contains(&tool) {
        bail!("MCP tool is not offered by this installed provider");
    }
    let response = session.request(
        3,
        "tools/call",
        json!({"name":tool,"arguments":args}),
    )?;
    if response["isError"] == true {
        return Ok(json!({
            "provider":id,"tool":tool,"ok":false,
            "result":response,"bridge_task_created":false
        }));
    }
    Ok(json!({
        "provider":id,"tool":tool,"ok":true,
        "result":response,"bridge_task_created":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn source_registration_is_local_and_explicit() {
        let root = std::env::temp_dir().join(format!(
            "lb-provider-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let executable = std::env::current_exe().unwrap();
        write_to(&root, "wechat", executable.to_str().unwrap(), &["serve".into()])
            .unwrap();
        let config = load_from(&root, "wechat").unwrap();
        assert_eq!(config.id, "wechat");
        assert_eq!(config.args, vec!["serve"]);
        assert!(write_to(&root, "wechat", executable.to_str().unwrap(), &[]).is_err());
        assert!(path_for(&root, "../bad").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rpc_results_and_user_arguments_are_validated() {
        assert!(parse_response(json!({"jsonrpc":"2.0","id":2,"result":{}}),2).is_ok());
        assert!(parse_response(json!({"jsonrpc":"2.0","id":1,"result":{}}),2).is_err());
        assert!(parse_response(json!({"jsonrpc":"2.0","id":2,"error":{"code":-1}}),2).is_err());
        assert!(read_json_input(r#"{"chat_id":"test","limit":5}"#).is_ok());
        assert!(read_json_input("[]").is_err());
        assert!(read_json_input("{bad}").is_err());
        assert!(parse_tool_name("wechat_send_message").is_ok());
        assert!(parse_tool_name("shell run").is_err());
    }

    #[test]
    fn detects_provider_errors_without_fake_success() {
        let result = json!({"isError":true,"content":[{"type":"text","text":"unavailable"}]});
        assert_eq!(result["isError"],true);
    }
}
