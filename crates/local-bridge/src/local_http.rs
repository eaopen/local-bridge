//! Local Bridge Web V2: loopback-only HTTP + SSE for existing MCP/Agent tasks.
//!
//! Chrome is the optional connection/consent bootstrap, not the data plane.
//! A short-lived token is created only by the trusted Native Host after an
//! explicit local OS-user site grant. No Profile, web page, or CORS preflight
//! can mint authorization. This service never binds to a LAN interface.
use crate::{agent_grants, agent_options, inbox, web_v2, workbuddy_web};
use anyhow::{bail, Context, Result};
use local_bridge_core::valid_origin;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const PORT: u16 = 48293;
const TTL_SEC: u64 = 600;
const MAX_HEADER_BYTES: usize = 8192;
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_CLIENTS: usize = 12;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    origin: String,
    expires_at: u64,
}
fn now() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
fn session_dir() -> Result<PathBuf> {
    Ok(inbox::data_root()?.join("web-v2-sessions"))
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn strong_token() -> Result<String> {
    // 256 bits of OS-provided randomness on both desktop platforms.
    // No timestamp, PID, browser-generated token or persisted shared secret.
    let mut raw = [0u8; 32];
    #[cfg(unix)]
    File::open("/dev/urandom")?.read_exact(&mut raw)?;
    #[cfg(windows)]
    {
        // BCryptGenRandom with a null algorithm handle uses Windows' system CSPRNG.
        // No new dependency or OS credential store is needed.
        #[link(name = "bcrypt")]
        unsafe extern "system" {
            fn BCryptGenRandom(
                algorithm: *mut std::ffi::c_void,
                buffer: *mut u8,
                buffer_size: u32,
                flags: u32,
            ) -> i32;
        }
        const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
        // SAFETY: raw is a valid writable 32-byte buffer; the null handle is
        // permitted with BCRYPT_USE_SYSTEM_PREFERRED_RNG.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                raw.as_mut_ptr(),
                raw.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            bail!("Windows system random generator unavailable");
        }
    }
    #[cfg(not(any(unix, windows)))]
    bail!("unsupported local session random generator");
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}
/// Plain 127.0.0.1 HTTP is the default. TLS is *not* required for a
/// same-machine Bridge, and Chrome LNA permission is distinct from TLS trust.
/// An operator-managed HTTPS reverse proxy is optional, with an OS/browser
/// trusted certificate. No auto CA install and no skipping certificate checks.
fn advertised_endpoint() -> Result<String> {
    let configured = std::env::var("LOCAL_BRIDGE_WEB_V2_HTTPS_ENDPOINT").unwrap_or_default();
    if configured.is_empty() {
        return Ok(format!("http://127.0.0.1:{PORT}"));
    }
    match configured.as_str() {
        "https://localhost:48294" | "https://127.0.0.1:48294" => Ok(configured),
        _ => bail!("unsupported HTTPS loopback proxy endpoint; TLS is optional"),
    }
}

fn token_id(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn token_file(token: &str) -> Result<PathBuf> {
    if !token_id(token) {
        bail!("invalid local session token");
    }
    Ok(session_dir()?.join(format!("s-{token}.json")))
}

/// Invoked by Chrome's trusted Native Host once per page; never by HTTP.
pub fn issue_session(origin: &str) -> Result<Value> {
    if !valid_origin(origin) || !(agent_grants::any(origin)? || web_v2::has_status_grant(origin)?) {
        bail!("OS user has not permitted this site to use local capabilities");
    }
    let endpoint = advertised_endpoint()?;
    let path = session_dir()?;
    private_dir(&path)?;
    // Lazy bounded cleanup at new bootstrap; never keep stale credentials forever.
    let now_time = now()?;
    let mut retained = 0;
    for file in fs::read_dir(&path)? {
        let item = file?;
        if !item.file_type()?.is_file() {
            continue;
        }
        let valid = fs::read(item.path())
            .ok()
            .and_then(|raw| serde_json::from_slice::<Session>(&raw).ok())
            .is_some_and(|record| record.expires_at > now_time);
        if valid {
            retained += 1;
        } else {
            let _ = fs::remove_file(item.path());
        }
    }
    if retained >= 128 {
        bail!("too many active local sessions");
    }
    for _ in 0..4 {
        let token = strong_token()?;
        let file = token_file(&token)?;
        let mut opts = OpenOptions::new();
        opts.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&file) {
            Ok(mut f) => {
                let body = serde_json::to_vec(&Session {
                    origin: origin.into(),
                    expires_at: now()? + TTL_SEC,
                })?;
                f.write_all(&body)?;
                f.sync_all()?;
                return Ok(json!({
                    "transport":"local-bridge-http-sse-v2",
                    "endpoint":endpoint,
                    "token":token,
                    "expires_in":TTL_SEC,
                    "scope":"local-bridge-v2-authorized-capabilities",
                }));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).context("cannot create local session"),
        }
    }
    bail!("could not allocate a unique local session")
}
fn verify_session(origin: &str, token: &str) -> Result<()> {
    if !(agent_grants::any(origin)? || web_v2::has_status_grant(origin)?) {
        bail!("site grant revoked");
    }
    let file = token_file(token)?;
    let metadata = fs::symlink_metadata(&file)?;
    if !metadata.file_type().is_file() || metadata.len() > 1024 {
        bail!("invalid local session");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!("unsafe local session file mode");
        }
    }
    let session: Session = serde_json::from_slice(&fs::read(&file)?)?;
    if session.origin != origin || session.expires_at <= now()? {
        bail!("local session expired or origin mismatch");
    }
    Ok(())
}

struct Request {
    method: String,
    path: String,
    origin: String,
    authorization: String,
    host: String,
    private_network: bool,
    content_type: String,
    body: Vec<u8>,
}
fn read_request(stream: &mut TcpStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    let mut bytes = Vec::new();
    let mut end = None;
    while bytes.len() < MAX_HEADER_BYTES {
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf)?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if let Some(i) = bytes.windows(4).position(|x| x == b"\r\n\r\n") {
            end = Some(i + 4);
            break;
        }
    }
    let stop = end.context("missing HTTP header terminator")?;
    if stop > MAX_HEADER_BYTES {
        bail!("request headers too large");
    }
    let text = std::str::from_utf8(&bytes[..stop])?;
    let mut lines = text.split("\r\n");
    let first = lines.next().context("missing HTTP request line")?;
    let request_line: Vec<&str> = first.split(' ').collect();
    if request_line.len() != 3
        || request_line[2] != "HTTP/1.1"
        || !matches!(request_line[0], "GET" | "POST" | "OPTIONS")
    {
        bail!("unsupported local HTTP request");
    }
    let mut headers = std::collections::BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').context("invalid header")?;
        let key = name.to_ascii_lowercase();
        if headers.insert(key, value.trim().to_owned()).is_some() {
            bail!("duplicate HTTP header");
        }
    }
    if headers.contains_key("transfer-encoding") {
        bail!("chunked requests not supported");
    }
    let length = headers
        .get("content-length")
        .map(|v| v.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    if length > MAX_REQUEST_BYTES {
        bail!("local request too large");
    }
    let mut body = bytes[stop..].to_vec();
    if body.len() > length {
        bail!("HTTP framing mismatch");
    }
    while body.len() < length {
        let mut chunk = [0u8; 2048];
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            bail!("truncated local HTTP body");
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() > length {
            bail!("HTTP body too long");
        }
    }
    Ok(Request {
        method: request_line[0].into(),
        path: request_line[1].into(),
        origin: headers.remove("origin").unwrap_or_default(),
        authorization: headers.remove("authorization").unwrap_or_default(),
        host: headers.remove("host").unwrap_or_default(),
        private_network: headers
            .get("access-control-request-private-network")
            .is_some_and(|x| x == "true"),
        content_type: headers.remove("content-type").unwrap_or_default(),
        body,
    })
}
fn safe_host(value: &str) -> bool {
    value == format!("127.0.0.1:{PORT}") || value == format!("localhost:{PORT}")
}
fn header_lines(origin: &str) -> String {
    format!(
        "Access-Control-Allow-Origin: {origin}\r\nVary: Origin\r\n\
        Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\n\
        Referrer-Policy: no-referrer\r\n"
    )
}
fn reply(stream: &mut TcpStream, code: u16, origin: Option<&str>, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value)?;
    let text = match code {
        200 => "OK",
        202 => "Accepted",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let cors = origin.map(header_lines).unwrap_or_default();
    write!(
        stream,
        "HTTP/1.1 {code} {text}\r\n\
        Content-Type: application/json\r\nContent-Length: {}\r\n\
        Connection: close\r\n{cors}\r\n",
        bytes.len()
    )?;
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}
fn cursor(path: &str) -> Result<u64> {
    let raw = path.split_once('?').map(|x| x.1).unwrap_or("");
    let query: Vec<_> = url::form_urlencoded::parse(raw.as_bytes()).collect();
    if query.len() != 1 || query[0].0 != "cursor" {
        bail!("expected one cursor");
    }
    Ok(query[0].1.parse::<u64>()?)
}
fn task_from_url(path: &str, suffix: &str) -> Option<String> {
    let clean = path.split('?').next()?;
    let rest = clean.strip_prefix("/v2/agents/tasks/")?;
    let task = rest.strip_suffix(suffix)?;
    if task.is_empty() || task.contains('/') {
        return None;
    }
    Some(task.to_owned())
}
fn parse_auth(header: &str) -> Result<&str> {
    let raw = header.strip_prefix("Bearer ").context("missing Bearer")?;
    if !token_id(raw) {
        bail!("invalid Bearer");
    }
    Ok(raw)
}
fn require_session(req: &Request) -> Result<()> {
    verify_session(&req.origin, parse_auth(&req.authorization)?)
}
fn preflight(stream: &mut TcpStream, req: &Request) -> Result<()> {
    let extra = if req.private_network {
        "Access-Control-Allow-Private-Network: true\r\n"
    } else {
        ""
    };
    let cors = header_lines(&req.origin);
    write!(
        stream,
        "HTTP/1.1 204 No Content\r\n\
      Content-Length: 0\r\nConnection: close\r\n{cors}\
      Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
      Access-Control-Allow-Headers: Authorization, Content-Type\r\n\
      Access-Control-Max-Age: 60\r\n{extra}\r\n"
    )?;
    stream.flush()?;
    Ok(())
}
fn stream_events(stream: &mut TcpStream, origin: &str, task_id: &str, from: u64) -> Result<()> {
    // No URL bearer tokens; the Authorization header was checked before this
    // function starts, and origin permission is rechecked for every poll.
    let cors = header_lines(origin);
    write!(
        stream,
        "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\nConnection: close\r\n{cors}\r\n"
    )?;
    stream.flush()?;
    let mut next = from;
    for _ in 0..30 {
        let result = agent_options::poll(origin, task_id, next)?;
        let events = result["events"]
            .as_array()
            .context("invalid worker events")?;
        for event in events {
            let seq = event["seq"].as_u64().context("missing seq")?;
            if seq <= next {
                bail!("duplicate or stale worker event");
            }
            let name = event["type"].as_str().context("missing event type")?;
            let serialized = serde_json::to_string(event)?;
            write!(stream, "id: {seq}\nevent: {name}\ndata: {serialized}\n\n")?;
            next = seq;
        }
        stream.flush()?;
        if result["terminal"] == true {
            // Final state is separate from a message.final; unknown/timeout may
            // not have a persisted task.completed or task.failed event.
            let status = result["status"].as_str().unwrap_or("unknown");
            let summary = serde_json::to_string(&json!({
                "type":"task.status","status":status,"terminal":true
            }))?;
            write!(stream, "event: task.status\ndata: {summary}\n\n")?;
            stream.flush()?;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
        if events.is_empty() {
            stream.write_all(b": ping\n\n")?;
            stream.flush()?;
        }
    }
    Ok(())
}
fn serve_client(mut stream: TcpStream) -> Result<()> {
    stream.set_write_timeout(Some(Duration::from_secs(20)))?;
    let req = match read_request(&mut stream) {
        Ok(value) => value,
        Err(_) => {
            let _ = reply(&mut stream, 400, None, &json!({"error":"bad_request"}));
            return Ok(());
        }
    };
    if !safe_host(&req.host) || !valid_origin(&req.origin) {
        reply(&mut stream, 403, None, &json!({"error":"invalid_origin"}))?;
        return Ok(());
    }
    // MCP-only sites are valid local clients too. Individual routes still
    // recheck exact WorkBuddy or MCP tool permissions before execution.
    let allowed = agent_grants::any(&req.origin).unwrap_or(false)
        || web_v2::has_status_grant(&req.origin).unwrap_or(false);
    if !allowed {
        reply(&mut stream, 403, None, &json!({"error":"site_not_granted"}))?;
        return Ok(());
    }
    if req.method == "OPTIONS" {
        preflight(&mut stream, &req)?;
        return Ok(());
    }
    if require_session(&req).is_err() {
        reply(
            &mut stream,
            401,
            Some(&req.origin),
            &json!({"error":"invalid_local_session"}),
        )?;
        return Ok(());
    }
    let path = req.path.split('?').next().unwrap_or("");
    match (req.method.as_str(), path) {
        ("GET", "/v2/capabilities") => {
            let value = json!({"protocol":"local-bridge-http-sse-v2",
                "workbuddy":workbuddy_web::capability(&req.origin)?,
                "agents":agent_options::capabilities(&req.origin)?,
                "mcp":web_v2::capabilities(&req.origin)?,
                "events":"sse+cursor", "local_bridge":true});
            reply(&mut stream, 200, Some(&req.origin), &value)?;
        }
        ("POST", "/v2/mcp/status") => {
            if !req.content_type.starts_with("application/json") {
                reply(
                    &mut stream,
                    400,
                    Some(&req.origin),
                    &json!({"error":"content_type"}),
                )?;
            } else {
                match serde_json::from_slice::<Value>(&req.body) {
                    Ok(data) if data.as_object().is_some_and(|o| o.len() == 2) => {
                        match (data["provider"].as_str(), data["tool"].as_str()) {
                            (Some(provider), Some(tool)) => {
                                match web_v2::status_tool(&req.origin, provider, tool) {
                                    Ok(value) => {
                                        reply(&mut stream, 200, Some(&req.origin), &value)?
                                    }
                                    Err(_) => reply(
                                        &mut stream,
                                        403,
                                        Some(&req.origin),
                                        &json!({"error":"local_mcp_tool_not_granted_or_unavailable"}),
                                    )?,
                                }
                            }
                            _ => reply(
                                &mut stream,
                                400,
                                Some(&req.origin),
                                &json!({"error":"invalid_tool"}),
                            )?,
                        }
                    }
                    _ => reply(
                        &mut stream,
                        400,
                        Some(&req.origin),
                        &json!({"error":"invalid_tool"}),
                    )?,
                }
            }
        }
        // Unified Agent entry point: Bridge resolves the caller's hints
        // before any Provider task starts. No vendor-specific V2 task endpoint.
        ("POST", "/v2/agents/tasks") => {
            if !req.content_type.starts_with("application/json") {
                reply(
                    &mut stream,
                    400,
                    Some(&req.origin),
                    &json!({"error":"content_type"}),
                )?;
            } else {
                match serde_json::from_slice::<Value>(&req.body)
                    .ok()
                    .and_then(|v| agent_options::from_value(&v).ok())
                {
                    Some(options) => match agent_options::run(&req.origin, options) {
                        Ok(task) => reply(&mut stream, 202, Some(&req.origin), &task)?,
                        Err(_) => reply(
                            &mut stream,
                            503,
                            Some(&req.origin),
                            &json!({"error":"no_authorized_compatible_agent"}),
                        )?,
                    },
                    None => reply(
                        &mut stream,
                        400,
                        Some(&req.origin),
                        &json!({"error":"invalid_agent_options"}),
                    )?,
                }
            }
        }
        ("GET", other) if task_from_url(other, "/events").is_some() => {
            let task = task_from_url(other, "/events").unwrap_or_default();
            match cursor(&req.path)
                .and_then(|cursor| agent_options::poll(&req.origin, &task, cursor))
            {
                Ok(value) => reply(&mut stream, 200, Some(&req.origin), &value)?,
                Err(_) => reply(
                    &mut stream,
                    404,
                    Some(&req.origin),
                    &json!({"error":"unknown_task_or_cursor"}),
                )?,
            }
        }
        ("GET", other) if task_from_url(other, "/stream").is_some() => {
            let task = task_from_url(other, "/stream").unwrap_or_default();
            if let Ok(seq) = cursor(&req.path) {
                if agent_options::poll(&req.origin, &task, seq).is_ok() {
                    let _ = stream_events(&mut stream, &req.origin, &task, seq);
                } else {
                    reply(
                        &mut stream,
                        404,
                        Some(&req.origin),
                        &json!({"error":"unknown_task"}),
                    )?;
                }
            } else {
                reply(
                    &mut stream,
                    400,
                    Some(&req.origin),
                    &json!({"error":"invalid_cursor"}),
                )?;
            }
        }
        _ => reply(
            &mut stream,
            404,
            Some(&req.origin),
            &json!({"error":"unknown_route"}),
        )?,
    }
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

/// Run only on loopback, independently of the native messaging short process.
/// A second instance fails to bind; no open LAN interfaces or wildcard CORS.
pub fn serve() -> Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, PORT))
        .context("cannot bind Local Bridge V2 loopback; already running?")?;
    let inflight = Arc::new(AtomicUsize::new(0));
    for incoming in listener.incoming() {
        let stream = incoming?;
        let count = Arc::clone(&inflight);
        if count.fetch_add(1, Ordering::AcqRel) >= MAX_CLIENTS {
            count.fetch_sub(1, Ordering::AcqRel);
            let _ = stream.shutdown(Shutdown::Both);
            continue;
        }
        thread::spawn(move || {
            let _ = serve_client(stream);
            count.fetch_sub(1, Ordering::AcqRel);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_is_cryptographically_random_and_strict() {
        let a = strong_token().unwrap();
        let b = strong_token().unwrap();
        assert!(token_id(&a));
        assert_ne!(a, b);
        assert!(!token_id("test"));
        assert!(!token_id("../etc/passwd"));
    }

    #[test]
    fn default_http_loopback_does_not_require_tls_certificate() {
        // The normal user path is HTTP on loopback, with a browser LNA grant
        // and Native-issued per-origin session token. No TLS by default.
        if std::env::var("LOCAL_BRIDGE_WEB_V2_HTTPS_ENDPOINT").is_err() {
            assert_eq!(advertised_endpoint().unwrap(), "http://127.0.0.1:48293");
        }
    }

    #[test]
    fn bound_host_and_cursor_are_strict() {
        assert!(safe_host(&format!("127.0.0.1:{PORT}")));
        assert!(!safe_host("attacker.example"));
        assert!(!safe_host(&format!("attacker.example:{}", PORT)));
        assert_eq!(
            cursor("/v2/agents/tasks/wb-1/events?cursor=24").unwrap(),
            24
        );
        assert!(cursor("/v2/agents/tasks/wb-1/events?cursor=x").is_err());
    }

    #[test]
    fn session_is_not_granted_from_remote_profile_or_unapproved_site() {
        assert!(issue_session("https://some-untrusted-origin.example.test").is_err());
    }
}
