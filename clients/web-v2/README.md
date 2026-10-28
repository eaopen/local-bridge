# Local Bridge Web Client V2

**Status:** experimental, source-level client for the V2 independent Web entry. The real Local Bridge + Chrome + signed-in Agent end-to-end chain still requires a DEV browser test.

## Why a build-time client, not a live JavaScript download

The enterprise Web application should bundle a **version-pinned, reviewed** JavaScript module (with the accompanying TypeScript declarations). There is no runtime `<script src="http://127.0.0.1/...">` injection and no executable code returned by a server Profile. Runtime discovery fetches only **JSON capability data** from the authorized local Bridge.

This lets an existing enterprise Web/SSO continue working even if no Local Bridge is installed, and prevents a local-service update from silently replacing production Web JavaScript. A future package registry can publish the same source; initial EAP integration uses a pinned vendored copy, not a new external npm runtime dependency.

## Minimal use — one Agent API for WorkBuddy / Codex / Pi

```js
import { createLocalBridgeClient } from './vendor/local-bridge-v2/client.js'

const bridge = createLocalBridgeClient()
const connection = await bridge.connect()
if (connection.available) {
  const task = await bridge.runAgent({
    apiVersion: 2,  // defaults to 2, rejects incompatible versions
    clientKey: 'a-unique-idempotency-key',
    prompt: 'Summarize user-selected synthetic notes',
    agent: 'auto', // or workbuddy | codex | pi
    agentMode: 'prefer', // require means no pre-submit fallback
    model: 'auto',
    thinkMode: 'auto'
  })
  // selected_agent is authoritative. Default model/thinkMode may be unknown.
  let cursor = 0
  let terminal = false
  while (!terminal) {
    const page = await bridge.readAgent(task.task_id, cursor, event => {
      if (event.type === 'message.delta') appendMessageChunk(event.message_id, event.text)
      if (event.type === 'message.final') markMessageFinal(event.message_id)
      if (event.type === 'task.status') updateTaskStatus(event.status)
    })
    cursor = page.next_seq
    terminal = page.terminal
  }
}
```

**Only WorkBuddy is currently a validated runnable Web V2 adapter** and its
default model/thinkMode cannot yet be overridden. Explicit unsupported model
or reasoning mode is rejected *before* execution. Codex/Pi are known Agent
IDs, not falsely advertised as Web-ready adapters. Provider selection and
ready checks belong to Bridge, not enterprise Web code. Web SDK V2 has no
`startWorkbuddy`/`readWorkbuddy`/`pollWorkbuddy` methods.

**Important:** `message.final` marks one assistant message, not the completion of an Agent task. Only a task terminal event/state confirms completion or failure. The stable `clientKey` is reused across uncertain direct HTTP → Native fallback submissions, so a transport failure must not start the Agent again.

## Transport and permissions

- Chrome V2 is used only for the initial exact-Origin, user-approved Native Host session; after that, **Local Bridge loopback HTTP/SSE is the primary data plane**. In local networking exceptions, the SDK uses short Chrome Native `agent.run/agent.read` calls as a compatibility fallback, preserving task ID/cursor.
- The local daemon owns MCP and Agent task execution and their event history. The SDK never stores the session token; it lives only in memory, travels via an Authorization header, never in URLs/query strings/Profile/LocalStorage.
- Session creation requires the local OS-user grant plus the separate browser site permission. Etech SSO, WorkBuddy login, personal MCP/WeChat and CloudFile ACL remain owned by their respective systems.
- SDK exposes the typed, versioned `runAgent/readAgent` contract; the currently runnable adapter is bounded WorkBuddy text and exact, locally authorized no-argument MCP status call; **no generic shell, file mutation, arbitrary provider command or WeChat message send**.
- Unsupported or absent Chrome, Host, WorkBuddy or Local Bridge resolves to `available: false`, leaving the original Web application unchanged.

## Integration / version management

Source: `clients/web-v2/client.js` and `client.d.ts`. No external runtime dependencies. `npm test` runs focused contract tests using Node's built-in test runner. For existing webpack/Vue2 applications, bundle the module's exact bytes at build time (first implementation: a checked-in `src/vendor/local-bridge-v2/client.js` copy), and record the exact Bridge commit SHA in the enterprise integration docs. New Bridge versions require an explicit downstream update, not runtime script injection.

**No duplicate protocol:** the SDK exports `chromeRequest`, `directJson` and `directStream` for compatibility, and `createLocalBridgeClient()` for the recommended high-level API. Do not implement a competing Origin/token/SSE parser in Etech.

**Deferred:** generic Agent Provider discovery/routing and WorkBuddy production Adapter, Windows/Mac installed-client E2E, WebSocket for interactive tool approval, desktop wrapper UI, CloudFile V1 migration. HTTP/SSE handles current one-way multi-message outputs.


## Unified Agent API (preview.3, `api_version=2`)

The Web client uses one Agent method for all providers. The execution choice is made in Local Bridge, never in enterprise Web application code:

~~~js
const task = await bridge.runAgent({
  apiVersion: 2,
  clientKey: 'a-stable-idempotency-key',
  prompt: 'Summarize the selected document',
  agent: 'workbuddy',   // optional preference; default 'auto'
  agentMode: 'prefer',  // optional 'prefer' | 'require'
  model: 'auto',        // optional exact model ID or 'auto'
  thinkMode: 'auto'     // optional: auto/off/low/medium/high/xhigh/max
})
// task.selected_agent is authoritative, not the requested agent.
// task.applied_model / applied_think_mode are null until verified.
const updates = await bridge.readAgent(task.task_id, 0, onAgentEvent)
~~~

Semantics:
- Missing agent uses the ApplicationProfile policy order and only currently available/authorized/compatible local adapters. Specifying an agent with agentMode=prefer raises its priority, but Bridge may select another ready adapter **before submission**. agentMode=require fails if the chosen provider is unavailable. The application does not maintain vendor-specific routing.
- Missing model/thinkMode asks the selected Provider to use its existing defaults. An explicitly selected model is an **exact constraint**, not a suggestion to silently switch models; an explicit reasoning level must be verifiably applied or be rejected. The actual model and think mode, if known, should appear in the returned applied fields; null means unverified provider defaults.
- clientKey is required and remains unchanged across HTTP errors and the Chrome Native fallback. Once accepted, fail/unknown never triggers automatic cross-provider retry. The same task_id/cursor is used to read multiple replies and independent task completion.
- **Current executable capability (2026-10-28)**: the merged Web worker supports WorkBuddy text-only with its own default model/think mode; Codex/Pi web run, explicit model selection and explicit think mode switching are NOT yet verified. The new generic request validates all three fields and rejects unsupported overrides before running. This is an intentionally honest API foundation, not a claim of 3 fully working adapters.
- The **unshipped** vendor-specific Web V2 SDK methods and V2 HTTP/Native task routes are removed. `runAgent/readAgent` is the only new public Web V2 Agent API. Existing CloudFile V1 and original YuanShu entry stay unchanged.


## Local loopback HTTP is the default; TLS is not required

- Default endpoint: `http://127.0.0.1:48293`, **without TLS**. Bridge listens on loopback only, validates Host/Origin, a per-origin OS-user grant and a Native-issued 10-minute bearer. No self-signed root certificates, automatic CA installation or TLS verification bypass.
- Chrome 142+ asks for Local Network Access permission on requests from secure public/intranet Web pages to loopback; Chrome 145+ splits it into `loopback-network` and `local-network`. With the Chrome loopback grant, HTTP requests to IP-literal loopback may be exempted from mixed-content blocking; an LNA denial or enterprise HTTP insecure-context policy falls back to **Chrome Native Messaging**, not a forced HTTPS upgrade.
- A user/company **may independently deploy a browser-trusted HTTPS reverse proxy** at `https://localhost:48294` (or `https://127.0.0.1:48294`) and set `LOCAL_BRIDGE_WEB_V2_HTTPS_ENDPOINT` in the trusted Native Host process. The proxy must terminate TLS with a certificate the browser *already trusts*, forward to `127.0.0.1:48293` and preserve the original `Origin` and short-lived bearer; for Host validation it must forward `Host: 127.0.0.1:48293`. This is **not** a Bridge default listener or certificate manager. It fails visibly if misconfigured.
- The SDK accepts only the fixed loopback HTTP addresses and the two optional fixed HTTPS proxy addresses from a trusted Native Host response. It never accepts a page-supplied local endpoint, arbitrary LAN address or proxy URL.
- Browsers control LNA and HTTPS certificate trust independently of Bridge. Native Messaging is the fallback for HTTPS/LNA/CORS failures. Use `targetAddressSpace:'loopback'` on direct requests where supported; unsupported browsers ignore this optional request hint.

Chrome references: [Chrome 142](https://developer.chrome.com/release-notes/142), [Chrome 145](https://developer.chrome.com/release-notes/145), [LNA guidance](https://developer.chrome.com/blog/local-network-access).
