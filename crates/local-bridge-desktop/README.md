# Local Bridge Desktop — optional local management UI (MVP)

**This is a separate Tauri v2 application, not the Local Bridge core CLI, not the original CloudFile Agent2 Tauri, and not an enterprise Web shell.** It is not built by the normal root workspace CI. The original Rust MCP/Native Host/Web V2 remains functional without any WebView/UI dependencies.

## In the first slice

One OS-user-local window with **Overview / Tasks / Settings**:
- Overview reads existing diagnostics, Bridge Native Host/MCP Provider registration, and Agent install hints; installed is not authenticated.
- Desktop startup automatically checks WorkBuddy, Codex and Pi once in sequence in the background. Overview and Agent settings retain a manual recheck button, including for missing/failed Agents after installation or login is fixed. Duplicate clicks share one queued check, failures do not stop the remaining checks, and page navigation/refresh does not trigger another automatic round. The current adapter runs one fixed synthetic text task and verifies completion plus its exact answer. Codex also checks CLI login status. Failed/unknown authentication is never inferred from installation; raw output and credentials are not stored. Results are session-local, timestamped, expire after five minutes, and do not grant any website access. A check may use a small amount of the Agent's existing model quota.
- Tasks render the existing task center for Codex/Pi/WorkBuddy/Inbox. Only an explicit user click includes redacted CloudFile file summaries via an installed Agent2 Provider. No file name/path/ticket, upload, checkin or background polling.
- Settings reuse the exact same AgentPreferences Rust get/save functions as the CLI for agent/model/think_mode/agent_mode/priority. No duplicate Tauri Store, model credentials, or enterprise AI billing.
- Settings have four always-visible categories: General, Agent, CloudFile and WeChat. General stores only appearance/start-page preferences in the desktop WebView. CloudFile/WeChat can register or remove their local connection with fixed provider arguments and explicitly discover tools. Registration, successful MCP discovery, and app login remain separate states. Missing providers show setup instructions; provider-owned configuration remains in the original application until it offers a settings interface.
- UI uses bundled static HTML/CSS/ES modules with a restrictive CSP, without a CDN or JS framework.

Local Tauri commands cover overview, tasks, Agent preferences, settings context and fixed CloudFile/WeChat provider registration/removal/tool discovery. Tool discovery runs outside the UI thread and never calls a chat/file tool. No HTTP management endpoint and no generic Web/MCP task-list permission.

Availability checks also run outside the UI thread via `desktop_agent_check`; only Codex, WorkBuddy and Pi are accepted. The CLI equivalent is `local-bridge agent check <codex|workbuddy|pi>`. A successful check means the exact checked configuration worked at that time, not a guarantee of future quota or unrelated models. WorkBuddy uses its existing macOS native-account text worker; no OAuth credential export or extra login is added. Pi failures leave authentication unknown unless a real task verifies it, and never trigger a model/provider fallback.

For DEV, set `LOCAL_BRIDGE_DATA_DIR` to an absolute Bridge-only state directory instead of replacing `HOME`. This keeps Bridge state separate while letting the existing Agents resolve their own login/configuration normally. It does not change the original Agent2 file data root or copy any credentials.

## Explicit development build

Isolated nested Cargo workspace: normal root CLI CI never compiles Tauri, WebView2 or its platform UI deps. With Rust/Tauri v2 host prerequisites installed:

```sh
cargo check --manifest-path crates/local-bridge-desktop/Cargo.toml
cargo run --manifest-path crates/local-bridge-desktop/Cargo.toml
node --test crates/local-bridge-desktop/ui/*.test.mjs
```

This is a **development MVP**, not a signed installer: bundle.active=false, no tray/autostart, and optional desktop Cargo.lock/release signing are not finalized. A later release task must pin/update its dependency tree and use a separate Local Bridge Desktop channel.

## Not in MVP

No Agent2 Tauri source copy, CloudFile Journal/Guard/data clone, global Shell or file tools, external navigation, task cancel/retry/commit, enterprise Web app shell, model API keys, automatic updater or system-wide resident daemon. The original CloudFile Chrome and Bridge Chrome keep separate Native Host IDs; the only CloudFile file owner stays Agent2.
