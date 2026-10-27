# Local Tool Bridge Chrome → Local Bridge Native Messaging V1

**Status:** Local Bridge provides an optional **separate** Chrome Native Messaging binary and a simple per-user local inbox. This is not CloudFile file editing, web SSO, or automatic AI processing. The independent Chrome extension continues to work without Local Bridge.

## 1. Build and install (user action, Chrome only)

Use a recent Rust toolchain (the repository pins 1.90). Build **both** binaries:

```sh
cargo build --workspace --bins
# Find the unpacked extension ID in chrome://extensions (or published ID).
# Must be exactly 32 letters in the a-p Chrome ID alphabet.
cargo run -p local-bridge -- native-host install <extension-id>
```

The installer locates `local-bridge-native-host` beside the executing CLI binary. When using `cargo run`, ensure both binaries were already built and are still installed together. For a release, place both executables in the same stable installation directory and run `local-bridge native-host install <extension-id>` from there. The installer writes the manifest only for the active OS user:

- macOS: `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.eaopen.browser_bridge.json`
- Linux/Chrome: `~/.config/google-chrome/NativeMessagingHosts/com.eaopen.browser_bridge.json`
- Windows: `%LOCALAPPDATA%\\LocalBridge\\NativeMessagingHosts\\com.eaopen.browser_bridge.json`, registered at HKCU `Software\\Google\\Chrome\\NativeMessagingHosts\\com.eaopen.browser_bridge`.

If a different manifest already exists, the command refuses to replace it. Only explicitly supply `--replace` after verifying the existing host. The allowed origin is exactly the supplied extension ID; no wildcard, no CloudFile host reuse. After installation, restart Chrome as needed.

In Local Tool Bridge popup, select the default provider `com.eaopen.browser_bridge` and click **Check provider**. It should report `provider=local-bridge` and `connected_to_local_bridge=true`. An independently installed third-party host remains a supported alternative.

## 2. User-directed page handoff

In the popup click **Capture**, inspect/edit the text, then **Send to local host**. Chrome `sendNativeMessage` uses 4-byte native-endian length + JSON. Supported requests:

- `host.status`: real native provider readiness and capabilities.
- `context.share`: bounded `{title,url,selection}` explicitly sent by the popup. Sanitized context is persisted to the **current OS user's local inbox**; never sent to Codex or CloudFile automatically.

A successful handoff returns `{received:true,stored:true,forwarded:false,status:"received",task_id:"ctx-..."}`. This means saved to an inbox, **not** an AI turn or file publish.

Use the local CLI to inspect/delete:

```sh
local-bridge inbox list
local-bridge inbox read ctx-...
local-bridge inbox delete ctx-...
```

Items are retained until the user deletes them. The inbox is created under the OS user data directory in `LocalBridge/inbox` (or `local-bridge/inbox` on Linux). On Unix the directory is private (0700), saved files private (0600). No browser token/cookie is collected or sent to the cloud by Bridge. Do not share confidential content unless it is appropriate for the user's machine and account.

The existing `local-bridge agent codex <prompt|->` is a **separate explicit user action**. To process an inbox item, the user can choose text in `inbox read` and explicitly pass it to an Agent. No automatic AI forwarding is present in Native Messaging V1.

## 3. Scope and compatibility

- Independent `eaopen/local-bridge-chrome-extension` and `eaopen/local-bridge` have separate release lifecycles. Neither is required for the other's base capabilities.
- Do not register `local-bridge mcp` as a Chrome host: MCP uses newline-delimited stdio, not framed Native Messaging.
- Existing CloudFile Chrome Extension and production host `com.cloudfile.current_agent` are unchanged. Three CloudFile local file modes remain with Agent2/Hub/Server.
- Web pages approved by the extension can only call read-only `extension.status`/`native.status`; they cannot call `context.share` or trigger an Agent. Only popup user interaction can send context.
- Future enterprise file/AI actions require scoped authorization and a versioned action contract; do not promote page text into arbitrary shell/EXE commands.

Tests include envelope validation, bounded framing, read-only status, URL filtering and a real local inbox file write. Browser native extension installation/roundtrip remains a separate on-device check; CI unit tests alone do not prove Chrome deployment.
