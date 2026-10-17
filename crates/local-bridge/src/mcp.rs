//! Deliberately small MCP stdio adapter. AI clients don't need an HTTP port,
//! administrator privileges, local model, browser extension or plugin runtime.
use crate::cloudfile;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

const PROTOCOL: &str = "2025-06-18";
const MAX_MESSAGE: usize = 1024 * 1024;

fn tool_content(value: Value, is_error: bool) -> Value {
    json!({"content":[{"type":"text","text":value.to_string()}],"isError":is_error})
}

fn tools() -> Value {
    json!([
        {
            "name":"bridge.status",
            "title":"Local Bridge status",
            "description":"Read-only local bridge version and capability information",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false},
            "annotations":{"readOnlyHint":true}
        },
        {
            "name":"cloudfile.status",
            "title":"CloudFile Agent2 component status",
            "description":"Read-only probe of the locally configured CloudFile Agent2; no file or credential access",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false},
            "annotations":{"readOnlyHint":true}
        }
    ])
}

fn execute(name: &str) -> Value {
    match name {
        "bridge.status" => tool_content(
            json!({
                "name":"local-bridge", "version":env!("CARGO_PKG_VERSION"),
                "mcp_transport":"stdio",
                "cloudfile_configured":cloudfile::configured(),
                "capabilities":local_bridge_core::AVAILABLE_CAPABILITIES
            }),
            false,
        ),
        "cloudfile.status" => tool_content(cloudfile::status(), false),
        _ => tool_content(
            json!({"error":"unknown or unapproved local capability"}),
            true,
        ),
    }
}

/// JSON-RPC result; notifications carry no response. This is only the MCP
/// methods actually implemented, rather than claiming a full gateway.
pub fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => {
            let requested = request["params"]["protocolVersion"]
                .as_str()
                .unwrap_or_default();
            let negotiated = if matches!(requested, "2025-03-26" | "2025-06-18" | "2025-11-25") {
                requested
            } else {
                PROTOCOL
            };
            json!({"protocolVersion":negotiated,
                   "capabilities":{"tools":{"listChanged":false}},
                   "serverInfo":{"name":"local-bridge","version":env!("CARGO_PKG_VERSION")}})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":tools()}),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or_default();
            let arguments = request["params"].get("arguments");
            if arguments.is_some_and(|v| !v.is_object())
                || arguments.is_some_and(|v| !v.as_object().is_some_and(|v| v.is_empty()))
            {
                tool_content(
                    json!({"error":"arguments not supported by this read-only tool"}),
                    true,
                )
            } else {
                execute(name)
            }
        }
        _ => {
            return Some(json!({"jsonrpc":"2.0","id":id,
            "error":{"code":-32601,"message":"method not implemented"}}))
        }
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

pub fn serve() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for input in stdin.lock().lines() {
        let input = input?;
        if input.is_empty() {
            continue;
        }
        if input.len() > MAX_MESSAGE {
            anyhow::bail!("MCP message size limit exceeded");
        }
        let output = match serde_json::from_str::<Value>(&input) {
            Ok(request) => handle(&request),
            Err(_) => Some(json!({"jsonrpc":"2.0","id":null,
                "error":{"code":-32700,"message":"invalid JSON"}})),
        };
        if let Some(value) = output {
            serde_json::to_writer(&mut stdout, &value)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initialize_tools_and_unknown_method() {
        let init = handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-06-18"}}))
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "local-bridge");
        let list = handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 2);
        let call = handle(&json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"bridge.status","arguments":{}}}))
        .unwrap();
        assert_eq!(call["result"]["isError"], false);
        let denied = handle(&json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"shell.run","arguments":{}}}))
        .unwrap();
        assert_eq!(denied["result"]["isError"], true);
        let unknown = handle(&json!({"jsonrpc":"2.0","id":5,"method":"arbitrary.run"})).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
    #[test]
    fn notifications_are_silent_and_arguments_are_restricted() {
        assert!(handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let denied = handle(&json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"bridge.status","arguments":{"command":"rm -rf"}}}))
        .unwrap();
        assert_eq!(denied["result"]["isError"], true);
    }
}
