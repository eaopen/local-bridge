# Local Bridge Web Client V2

**Status:** experimental, source-level client for the V2 independent Web entry. The real Local Bridge + Chrome + signed-in WorkBuddy end-to-end chain still requires a DEV browser test.

## Why a build-time client, not a live JavaScript download

The enterprise Web application should bundle a **version-pinned, reviewed** JavaScript module (with the accompanying TypeScript declarations). There is no runtime `<script src="http://127.0.0.1/...">` injection and no executable code returned by a server Profile. Runtime discovery fetches only **JSON capability data** from the authorized local Bridge.

This lets an existing enterprise Web/SSO continue working even if no Local Bridge is installed, and prevents a local-service update from silently replacing production Web JavaScript. A future package registry can publish the same source; initial EAP integration uses a pinned vendored copy, not a new external npm runtime dependency.

## Minimal use

```javascript
import {createLocalBridgeClient} from './vendor/local-bridge-v2/client.js'

const bridge = createLocalBridgeClient()
const connection = await bridge.connect()  // never breaks the parent Web page
if (connection.available && connection.capabilities?.workbuddy?.installed) {
  const task = await bridge.startWorkbuddy({
    clientKey: 'a-unique-idempotency-key',
    prompt: 'Summarize these user-selected notes'
  })
  let cursor = 0
  let terminal = false
  while (!terminal) {
    const result = await bridge.readWorkbuddy(task.task_id, cursor, event => {
      if (event.type === 'message.delta') appendMessageChunk(event.message_id, event.text)
      if (event.type === 'message.final') markMessageFinal(event.message_id)
      if (event.type === 'task.status') updateTaskStatus(event.status)
    })
    cursor = result.next_seq
    terminal = result.terminal
  }
}
```

**Important:** `message.final` marks one assistant message, not the completion of an Agent task. Only a task terminal event/state confirms completion or failure. The stable `clientKey` is reused across uncertain direct HTTP → Native fallback submissions, so a transport failure must not start the Agent again.

## Transport and permissions

- Chrome V2 is used only for the initial exact-Origin, user-approved Native Host session; after that, **Local Bridge loopback HTTP/SSE is the primary data plane**. In local networking exceptions, the SDK uses short Chrome Native `agent.start/agent.poll` calls as a compatibility fallback, preserving task ID/cursor.
- The local daemon owns MCP and Agent task execution and their event history. The SDK never stores the session token; it lives only in memory, travels via an Authorization header, never in URLs/query strings/Profile/LocalStorage.
- Session creation requires the local OS-user grant plus the separate browser site permission. Etech SSO, WorkBuddy login, personal MCP/WeChat and CloudFile ACL remain owned by their respective systems.
- SDK explicitly exposes only a bounded WorkBuddy text task and exact, locally authorized no-argument MCP status call; **no generic shell, file mutation, arbitrary provider command or WeChat message send**.
- Unsupported or absent Chrome, Host, WorkBuddy or Local Bridge resolves to `available: false`, leaving the original Web application unchanged.

## Integration / version management

Source: `clients/web-v2/client.js` and `client.d.ts`. No external runtime dependencies. `npm test` runs focused contract tests using Node's built-in test runner. For existing webpack/Vue2 applications, bundle the module's exact bytes at build time (first implementation: a checked-in `src/vendor/local-bridge-v2/client.js` copy), and record the exact Bridge commit SHA in the enterprise integration docs. New Bridge versions require an explicit downstream update, not runtime script injection.

**No duplicate protocol:** the SDK exports `chromeRequest`, `directJson` and `directStream` for compatibility, and `createLocalBridgeClient()` for the recommended high-level API. Do not implement a competing Origin/token/SSE parser in Etech.

**Deferred:** generic Agent Provider discovery/routing and WorkBuddy production Adapter, Windows/Mac installed-client E2E, WebSocket for interactive tool approval, desktop wrapper UI, CloudFile V1 migration. HTTP/SSE handles current one-way multi-message outputs.


## Unified Agent API (preview.2)

The Web client uses one Agent method for all providers. The execution choice is made in Local Bridge, never in enterprise Web application code:

~~~js
const task = await bridge.runAgent({
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
- Old startWorkbuddy/readWorkbuddy remain compatible and continue to use the existing endpoints; this addition does not change CloudFile V1 or the original YuanShu entry.
