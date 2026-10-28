# Local Bridge Desktop — optional local management UI (MVP)

**This is a separate Tauri v2 application, not the Local Bridge core CLI, not the original CloudFile Agent2 Tauri, and not an enterprise Web shell.** It is not built by the normal root workspace CI. The original Rust MCP/Native Host/Web V2 remains functional without any WebView/UI dependencies.

## In the first slice

One OS-user-local window with **Overview / Tasks / Settings**:
- Overview reads existing diagnostics, Bridge Native Host/MCP Provider registration, and Agent install hints; installed is not authenticated.
- Tasks render the existing task center for Codex/Pi/WorkBuddy/Inbox. Only an explicit user click includes redacted CloudFile file summaries via an installed Agent2 Provider. No file name/path/ticket, upload, checkin or background polling.
- Settings reuse the exact same AgentPreferences Rust get/save functions as the CLI for agent/model/think_mode/agent_mode/priority. No duplicate Tauri Store, model credentials, or enterprise AI billing.
- UI uses bundled static HTML/CSS/ES modules with a restrictive CSP, without a CDN or JS framework.

Four local Tauri commands: desktop_overview, desktop_tasks, desktop_preferences_get, desktop_preferences_save. No HTTP management endpoint and no generic Web/MCP task-list permission.

## Explicit development build

Isolated nested Cargo workspace: normal root CLI CI never compiles Tauri, WebView2 or its platform UI deps. With Rust/Tauri v2 host prerequisites installed:

```sh
cargo check --manifest-path crates/local-bridge-desktop/Cargo.toml
cargo run --manifest-path crates/local-bridge-desktop/Cargo.toml
node --test crates/local-bridge-desktop/ui/preferences.test.mjs
```

This is a **development MVP**, not a signed installer: bundle.active=false, no tray/autostart, and optional desktop Cargo.lock/release signing are not finalized. A later release task must pin/update its dependency tree and use a separate Local Bridge Desktop channel.

## Not in MVP

No Agent2 Tauri source copy, CloudFile Journal/Guard/data clone, global Shell or file tools, external navigation, task cancel/retry/commit, enterprise Web app shell, model API keys, automatic updater or system-wide resident daemon. The original CloudFile Chrome and Bridge Chrome keep separate Native Host IDs; the only CloudFile file owner stays Agent2.
