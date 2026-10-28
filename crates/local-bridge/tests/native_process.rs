//! Exercise the two *real* binaries over Chrome's byte-framed stdio.
//! This is a subprocess contract test, not a claim of a Chrome UI E2E.
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn for_user(binary: &str, root: &Path) -> Command {
    let mut cmd = Command::new(binary);
    cmd.env("HOME", root)
        .env("LOCALAPPDATA", root)
        .env("XDG_DATA_HOME", root);
    cmd
}

fn send_native(request: Value, root: &Path) -> Value {
    let mut child = for_user(env!("CARGO_BIN_EXE_local-bridge-native-host"), root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let bytes = serde_json::to_vec(&request).unwrap();
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(&(bytes.len() as u32).to_ne_bytes())
        .unwrap();
    input.write_all(&bytes).unwrap();
    drop(input);

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "native host did not exit successfully"
    );
    assert!(output.stdout.len() >= 4, "no Chrome native frame");
    let prefix: [u8; 4] = output.stdout[..4].try_into().unwrap();
    let length = u32::from_ne_bytes(prefix) as usize;
    assert_eq!(output.stdout.len(), 4 + length);
    serde_json::from_slice(&output.stdout[4..]).unwrap()
}

fn command(root: &Path, args: &[&str]) -> Value {
    let output = for_user(env!("CARGO_BIN_EXE_local-bridge"), root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "inbox CLI returned failure");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn native_host_status_and_explicit_inbox_handoff() {
    let root = std::env::temp_dir().join(format!(
        "local-bridge-native-process-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();

    // A Bridge CLI installation can be healthy without the optional Chrome Host.
    // The doctor does not connect to a real Chrome UI or read CloudFile tasks.
    let initial = command(&root, &["native-host", "status"]);
    assert_eq!(initial["state"], "not_installed");
    assert_eq!(initial["browser_verified"], false);
    let doctor = command(&root, &["self-test"]);
    assert_eq!(doctor["ok"], true);
    assert_eq!(doctor["chrome_gui_verified"], false);
    assert_eq!(doctor["agent_authentication_verified"], false);
    assert_eq!(doctor["cloudfile_file_execution_tested"], false);

    let status = send_native(
        json!({
            "protocol_version":1,"request_id":"status_1",
            "action":"host.status","payload":{}
        }),
        &root,
    );
    assert_eq!(status["ok"], true);
    assert_eq!(status["result"]["provider"], "local-bridge");

    let shared = send_native(
        json!({
            "protocol_version":1,"request_id":"share_1",
            "action":"context.share",
            "payload":{
                "title":"Project note",
                "url":"https://example.org/project",
                "selection":"User-selected example text"
            }
        }),
        &root,
    );
    assert_eq!(shared["ok"], true);
    assert_eq!(shared["result"]["stored"], true);
    assert_eq!(shared["result"]["forwarded"], false);
    let id = shared["result"]["task_id"].as_str().unwrap();

    let listing = command(&root, &["inbox", "list"]);
    assert_eq!(listing["items"][0]["id"], id);
    let item = command(&root, &["inbox", "read", id]);
    assert_eq!(item["context"]["selection"], "User-selected example text");
    let relay = send_native(
        json!({
            "protocol_version":1,"request_id":"relay_1",
            "action":"relay.return",
            "payload":{
                "relay_id":"rel-0123456789abcdef0123456789abcdef",
                "task_ref":id,
                "web_ai":"chatgpt",
                "source":{"title":"Project note","url":"https://example.org/project"},
                "reply":{"title":"ChatGPT","url":"https://chatgpt.com",
                         "selection":"Suggested changes, not executed."}
            }
        }),
        &root,
    );
    assert_eq!(relay["ok"], true);
    assert_eq!(relay["result"]["status"], "received");
    let returned_id = relay["result"]["task_id"].as_str().unwrap();
    let returned = command(&root, &["inbox", "read", returned_id]);
    assert_eq!(returned["source"], "web-ai-relay");
    assert_eq!(returned["relay"]["task_ref"], id);
    assert_eq!(
        returned["context"]["selection"],
        "Suggested changes, not executed."
    );
    assert_eq!(
        command(&root, &["inbox", "delete", returned_id])["deleted"],
        true
    );
    let prepared = command(&root, &["relay", "prepare", id]);
    assert_eq!(prepared["task_ref"], id);
    assert!(prepared["prompt"]
        .as_str()
        .unwrap()
        .contains("User-selected example text"));
    assert_eq!(command(&root, &["inbox", "delete", id])["deleted"], true);
    assert!(command(&root, &["inbox", "list"])["items"]
        .as_array()
        .unwrap()
        .is_empty());

    std::fs::remove_dir_all(root).unwrap();
}
