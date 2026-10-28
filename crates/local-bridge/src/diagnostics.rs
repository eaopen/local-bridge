//! Small non-invasive local health report, following Agent2's self-test
//! principles: distinguish optional/unconfigured from genuinely broken.
use crate::{inbox, mcp_provider, native_host_registration};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn check(name: &str, state: &str, ok: bool) -> Value {
    json!({"name":name,"state":state,"ok":ok})
}
fn data_check() -> Value {
    let result = (|| -> anyhow::Result<()> {
        let dir = inbox::data_root()?;
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let file = dir.join(format!(".local-bridge-self-test-{}-{at}", std::process::id()));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut out = opts.open(&file)?;
        let result = (|| -> anyhow::Result<()> {
            out.write_all(b"local-bridge-self-test")?;
            out.sync_all()?;
            let back = fs::read(&file)?;
            anyhow::ensure!(back == b"local-bridge-self-test", "local state readback failed");
            Ok(())
        })();
        drop(out);
        let _ = fs::remove_file(file);
        result
    })();
    check(
        "private-state",
        if result.is_ok() { "read_write_ok" } else { "unavailable" },
        result.is_ok(),
    )
}
pub fn run() -> Value {
    let data = data_check();
    let host = match native_host_registration::status() {
        Ok(state) => {
            let host_state = state["state"].as_str().unwrap_or("unknown");
            check(
                "native-host-manifest",
                host_state,
                matches!(host_state, "not_installed" | "manifest_ready_browser_not_verified"),
            )
        }
        Err(_) => check("native-host-manifest", "invalid_or_unreadable", false),
    };
    let providers = match mcp_provider::list() {
        Ok(list) => {
            let count = list["providers"].as_array().map_or(0, Vec::len);
            json!({"name":"mcp-provider-registry","state":"readable","ok":true,
                "registered_count":count,"login_verified":false})
        }
        Err(_) => check("mcp-provider-registry", "unavailable", false),
    };
    let checks = vec![data, host, providers];
    let ok = checks.iter().all(|c| c["ok"] == true);
    json!({
        "name":"local-bridge",
        "version":env!("CARGO_PKG_VERSION"),
        "ok":ok,
        "checks":checks,
        "chrome_gui_verified":false,
        "agent_authentication_verified":false,
        "cloudfile_file_execution_tested":false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_does_not_confuse_unconfigured_with_unhealthy() {
        assert_eq!(check("x", "not_installed", true)["ok"], true);
        assert_eq!(check("x", "invalid_or_unreadable", false)["ok"], false);
    }
}
