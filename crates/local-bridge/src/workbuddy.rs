//! Explicit CLI-only access to WorkBuddy's official Local Assistant API.
//! Credentials come from an authorized application's access token, never app storage.
use anyhow::{bail, Context, Result};
use reqwest::blocking::Client as HttpClient;
use reqwest::{header, redirect, Method};
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

const BASE: &str = "https://www.workbuddy.cn/openapi/v2/localassistant";
const MAX_RESPONSE: u64 = 4 * 1024 * 1024;

pub struct Client {
    http: HttpClient,
    base: String,
}

impl Client {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("LOCAL_BRIDGE_WORKBUDDY_ACCESS_TOKEN").context(
            "WorkBuddy OAuth access token missing; authorize a third-party application first",
        )?;
        Self::new(&token, BASE)
    }

    fn new(token: &str, base: &str) -> Result<Self> {
        if token.trim().is_empty() {
            bail!("WorkBuddy access token is empty");
        }
        let mut auth = header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| anyhow::anyhow!("invalid WorkBuddy access token"))?;
        auth.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, auth);
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        let http = HttpClient::builder()
            .default_headers(headers)
            .redirect(redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self {
            http,
            base: base.to_owned(),
        })
    }

    fn request(&self, method: Method, body: Option<Value>, cursor: Option<&str>) -> Result<Value> {
        let writing = method == Method::POST;
        let url = if writing || cursor.is_some() {
            format!("{}/message", self.base)
        } else {
            self.base.clone()
        };
        let mut request = self.http.request(method, url);
        if let Some(body) = body {
            request = request.json(&body);
        }
        if let Some(id) = cursor {
            request = request.query(&[("message_id", id)]);
        }
        // Never retry a POST: an interrupted response may already have started execution.
        let response = request.send().map_err(|_| {
            anyhow::anyhow!(
                "WorkBuddy transport error; outcome unknown; do not automatically resend"
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            bail!("WorkBuddy HTTP {}; check OAuth expiry/scopes for 401/403; do not automatically resend", status.as_u16());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                anyhow::anyhow!("WorkBuddy response interrupted; outcome unknown; do not resend")
            })?;
        if bytes.len() as u64 > MAX_RESPONSE {
            bail!("WorkBuddy response too large; outcome unknown");
        }
        decode(&bytes)
    }

    pub fn status(&self) -> Result<Value> {
        let data = self.request(Method::GET, None, None)?;
        let online = data["online"]
            .as_bool()
            .context("WorkBuddy response missing online flag")?;
        Ok(
            json!({"provider":"workbuddy","api_ready":true,"read_authorized":true,
            "invoke_authorized":null,"online":online}),
        )
    }

    pub fn send(&self, prompt: &str) -> Result<Value> {
        if prompt.trim().is_empty() || prompt.len() > 65536 {
            bail!("prompt must contain 1..65536 bytes");
        }
        if self.status()?["online"] != true {
            bail!("WorkBuddy local assistant is offline; no message sent");
        }
        let data = self.request(
            Method::POST,
            Some(json!({"content":prompt,"msg_type":"text"})),
            None,
        )?;
        let id = data["message_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("WorkBuddy submission outcome unknown: missing message_id; do not resend")?;
        Ok(
            json!({"provider":"workbuddy","owner_module":"ai.workbuddy","status":"accepted",
            "message_id":id,"completed":false,"persisted_by_bridge":false}),
        )
    }

    pub fn messages(&self, id: &str) -> Result<Value> {
        if id.trim().is_empty() || id.len() > 1024 {
            bail!("invalid message ID");
        }
        let data = self.request(Method::GET, None, Some(id))?;
        let messages = data["messages"]
            .as_array()
            .context("WorkBuddy response missing messages")?;
        // History is account-wide after this cursor, not a correlated task result.
        Ok(
            json!({"provider":"workbuddy","after_message_id":id,"status":"unknown",
            "completion_verified":false,"messages":messages,"persisted_by_bridge":false}),
        )
    }
}

fn decode(bytes: &[u8]) -> Result<Value> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| anyhow::anyhow!("invalid WorkBuddy response; outcome unknown"))?;
    if value["code"].as_i64() != Some(0) {
        bail!("WorkBuddy API rejected request or returned an invalid envelope; verify authorization; no automatic retry");
    }
    if !value["data"].is_object() {
        bail!("WorkBuddy response missing data; outcome unknown");
    }
    Ok(value["data"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_errors_and_malformed_envelopes_without_echoing_secrets() {
        for body in [
            r#"{"code":403,"msg":"secret"}"#,
            r#"{"data":{}}"#,
            "not json",
            r#"{"code":0}"#,
        ] {
            let err = decode(body.as_bytes()).unwrap_err().to_string();
            assert!(!err.contains("secret"));
        }
        assert_eq!(
            decode(br#"{"code":0,"data":{"online":false}}"#).unwrap()["online"],
            false
        );
    }
    #[test]
    fn invalid_credentials_are_not_printed() {
        assert!(Client::new("", BASE).is_err());
        assert!(Client::new("secret\n", BASE)
            .err()
            .unwrap()
            .to_string()
            .contains("invalid WorkBuddy"));
    }
    #[test]
    fn real_http_submission_and_history_do_not_claim_completion() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/localassistant", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (method, body) in [
                (
                    "GET /localassistant ",
                    r#"{"code":0,"data":{"online":true}}"#,
                ),
                (
                    "POST /localassistant/message ",
                    r#"{"code":0,"data":{"message_id":"msg-1"}}"#,
                ),
                (
                    "GET /localassistant/message?message_id=msg-1 ",
                    r#"{"code":0,"data":{"messages":[{"role":"assistant","content":["answer"]}]}}"#,
                ),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                assert!(first.starts_with(method), "{first}");
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(n) = line.to_lowercase().strip_prefix("content-length:") {
                        length = n.trim().parse::<usize>().unwrap();
                    }
                }
                let mut payload = vec![0; length];
                reader.read_exact(&mut payload).unwrap();
                if method.starts_with("POST") {
                    assert_eq!(
                        serde_json::from_slice::<Value>(&payload).unwrap(),
                        json!({"content":"test","msg_type":"text"})
                    );
                }
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let client = Client::new("test-token", &base).unwrap();
        let submitted = client.send("test").unwrap();
        assert_eq!(submitted["status"], "accepted");
        assert_eq!(submitted["completed"], false);
        let history = client.messages("msg-1").unwrap();
        assert_eq!(history["status"], "unknown");
        assert_eq!(history["completion_verified"], false);
        server.join().unwrap();
    }
}
