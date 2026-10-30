# Ecosystem Integration V1 — reuse before installation

Status: initial implementation, **not a production-released integration**. This contract describes only behavior implemented by the corresponding PR and clearly separates planned future adapters.

## Decision

Local Bridge is not an alternative launcher, input method, plugin store, model router, or AI runtime. Prefer mature ecosystems and existing permissions.

1. **Passive discovery:** \`local-bridge ecosystem discover\` identifies possible existing CLI/app installations. No model runs, network scan, vendor-profile access, MCP call, login or software installation. App-path detection is best effort and can miss nonstandard installs.
2. **Zero additional registration for installed Agents:** Codex/WorkBuddy/Pi continue through existing \`AgentPreferences\` and \`runAgent/readAgent\`. Installed != usable: the explicit user-initiated availability check still verifies account/model/API.
3. **One click when the user wants a change:** Tauri **Settings → 生态接入** and the OS-user CLI expose exactly two fixed actions:
   - \`ecosystem connect codex\`: use **official Codex CLI** to add \`local-bridge mcp\` to the current OS user's Codex configuration if absent, and refuse to overwrite an existing entry with different details. Does not configure a model or grant access.
   - \`ecosystem install pi\`: only if missing, **Node.js >= 22.19** and npm are available; executes \`npm install --global --ignore-scripts @earendil-works/pi-coding-agent\` without invoking a shell. Does not execute \`pi /login\`, install Node.js, add a provider or store credentials. CLI PATH may require a refresh.
4. **No silent global changes:** Installation, vendor config writes, API key usage, permission grant, plugin installation and high-risk tool calls are not part of discovery. Existing other software is not modified behind the user's back.
5. **Best-effort UI discovery:** Mac app bundles / selected Windows installation paths / select Linux applications, without reading foreign application databases or scanning ports. \`connection_verified=false\` is deliberate until a real protocol handshake succeeds.

## uTools as an optional candidate, but not an Agent yet

Official uTools 8 docs provide:
- \`utools.registerTool()\` and an MCP Server exposing installed plugin tools to external MCP clients. The user can enable the MCP service in **uTools Settings → AI → MCP**; server port can be customized.
- \`utools.ai()\` inside a uTools **plugin**, using configured or built-in uTools model services. It is a useful *possible model source* for an explicitly installed and authorized plugin, not a standalone public model API.
- Neither the uTools application icon nor MCP tool discovery proves that model access, pricing/AI-energy balance, plugin availability or Agent tool support is ready.

Therefore \`utools\` is reported as \`tool_host_and_optional_ai_source\`, **not** added to the WorkBuddy/Codex/Pi AgentPreferences or auto-selected as a fourth Agent. We should implement a small uTools plugin and its MCP/AI bridge only after verifying the real uTools plugin environment, permission prompts, model/energy usage and transport. Avoid guessing a default MCP port or introspecting private uTools config. No access to arbitrary models via \`utools.ai\` is assumed.

## Two eligible integration styles

| Installed platform | Approach | First delivery |
| --- | --- | --- |
| Codex CLI | Use existing Agent; user can one-click register Bridge MCP via official \`codex mcp\` | Implemented |
| WorkBuddy | Reuse actual available desktop/approved API adapter; no duplicate install | Existing adapter only |
| Pi | Reuse existing installation first; explicit one-click npm install if missing and prerequisites met | Implemented |
| uTools | Reuse its own plugin library and MCP Server; AI only after an authorized uTools plugin | Discovered, runtime integration **not** implemented |
| Raycast | Use built-in **Install MCP Server** (stdio); no private config injection | Detected, user-side connection still needed |
| Quicker | Reuse actions/MCP via official integration | Detected, Bridge interoperability pending |
| macOS Shortcuts / PowerToys | Reuse OS or existing entry points rather than new input method | Detected |
| ChatGPT/Gemini/Claude Web UI | Existing manual relay, not a headless API for Pi | Existing manual fallback |

## Minimal verification

\`\`\`sh
cargo test -p local-bridge ecosystem
node --check crates/local-bridge-desktop/ui/app.mjs
node --test crates/local-bridge-desktop/ui/*.test.mjs
local-bridge ecosystem discover
# Explicit opt-in only:
local-bridge ecosystem connect codex
local-bridge ecosystem install pi
\`\`\`

An app being found (or an npm command exiting successfully) is not proof of a usable model, MCP tool or successful run. Real native GUI and uTools MCP/AI tests must be done on a machine with those products installed.

Sources: [uTools MCP](https://www.u-tools.cn/docs/development/mcp.html) · [uTools AI](https://www.u-tools.cn/docs/api-reference/ai.html) · [Codex MCP CLI](https://learn.chatgpt.com/docs/cli/reference) · [Pi installation](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/README.md) · [Raycast MCP](https://manual.raycast.com/ai/model-context-protocol).
