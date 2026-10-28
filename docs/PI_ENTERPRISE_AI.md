# Enterprise AI as Pi's model — no per-user Pi model setup

**Goal:** A user already signed into an enterprise application can use Pi as a local Agent even without a personal Pi subscription, DeepSeek key or OpenCode model setup. **The enterprise app remains the provider of inference, identity, authorization, quota and billing.** Local Bridge is not a model gateway, 9Router alternative, credential vault or replacement for the enterprise AI backend.

## Implemented: minimal per-invocation Pi model binding

Pi supports compatible Chat Completions endpoints via `models.json`. Bridge generates an **isolated, temporary** `PI_CODING_AGENT_DIR` containing only one `local-bridge-enterprise` model entry and an environment-variable reference to a short-lived credential. It never modifies the user's normal `~/.pi/agent`, creates permanent provider settings or puts a token into `models.json`.

A **trusted local launcher / already-authorized enterprise application** can provision this process environment:

```sh
# Supplied by the authenticated enterprise app/service, not by an end user:
# LOCAL_BRIDGE_ENTERPRISE_AI_BASE_URL=https://ai.enterprise.example/v1
# LOCAL_BRIDGE_ENTERPRISE_AI_MODEL=team-chat
# LOCAL_BRIDGE_ENTERPRISE_AI_TOKEN=<short-lived scoped authorization>

local-bridge agent pi enterprise "Only reply ENTERPRISE_PI_OK"
printf '%s' 'A selected task approved by the enterprise app' |
  local-bridge agent pi enterprise -
```

The real URL, model and token are issued by the **enterprise application**. These are example variable names, not an invitation to put API credentials in a shell script. Bridge validates endpoint (HTTPS, or loopback HTTP for local testing), model syntax and token presence. Invalid/missing session fails visibly; it never falls back to a personal Pi key or an external model behind the user's back.

Pi is launched for a **single no-file-tools text task**, with no external extensions, no persistent session and no user model settings. The server must support OpenAI-compatible `/chat/completions` streaming. Enterprise API fees/limits remain governed by the original service; zero display-only Pi cost metadata **does not imply free model service**.

### Verified

On a real macOS Pi CLI process, an isolated `PI_CODING_AGENT_DIR` exposed a corporate model without touching the user's Pi configuration. Against a **synthetic local OpenAI-compatible SSE server**, Pi sent the chosen model, correct `Authorization: Bearer` header, and `stream:true` to `/v1/chat/completions`. The returned `ENTERPRISE_PI_OK` was produced successfully. This verifies Pi's wire compatibility and ephemeral configuration; **it is not a live enterprise server or SSO acceptance test**.

### Not yet integrated

1. Trusted enterprise Web/profile/SSO issuing a short-lived, scope-bound Pi model ticket via an already-logged-in application's backend, delivered to a registered local caller. The current `validate-profile` is shape-only and **cannot** authenticate a company endpoint by itself. Never allow untrusted Web pages or model text to choose the base URL or token.
2. Authentic enterprise model endpoint, actual account authorization and billing behavior. Without enterprise session or Pi installed, report unavailable; don't claim success.
3. In a future trusted Web → Bridge → Pi task, return task state/result to the originating page without replaying unknown file/tool side effects.

If the enterprise service already speaks another supported Pi API, prefer its native Pi provider/config at the application-owned integration layer. The Bridge V1 slice deliberately supports only OpenAI-compatible Chat Completions to stay small.

## Chrome Web AI Relay is different

The independent [Chrome Web AI Relay](https://github.com/eaopen/local-bridge-chrome-extension/blob/dev/docs/WEB_AI_RELAY.md) helps users reuse existing ChatGPT/Gemini/Claude Web login **with active human participation**. It can copy a prepared task, open Web Chat, and bring a user-selected answer back to a Bridge inbox. It cannot reliably provide unattended multi-turn model callbacks, structured tool calls, quotas or automatic completion to Pi without a supported service API.

**Do not create a background ChatGPT Web scraper, headless consumer-chat API, cookie exporter or free-quota bypass.** Where a service offers a supported API, use it with the appropriate authorization. Where it does not, expose `needs_action` and a low-friction user handoff, rather than marking Pi ready when the model is unavailable.

The feature is optional. Normal Codex/WorkBuddy, Pi with the user's own models, CloudFile Agent2, and an extension-only Relay continue to work independently.
