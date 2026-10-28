# Web AI Relay V1 — Manual browser chat handoff, not a model gateway

**Status:** Optional user-driven handoff. It is **not** a 9Router alternative, an API model provider, an automatic browser agent or a way to convert consumer chat quotas into an API. Web chat remains under the user's own login and platform limits. The Chrome extension works without Local Bridge.

## A. From the Chrome extension

1. The user selects or edits text in the popup, chooses a Web AI destination (ChatGPT/Gemini/Claude/Copilot/DeepSeek or a user-entered HTTPS chat URL), optionally adds an existing local task reference, then clicks **Copy task + open Web AI**.
2. The extension copies a short task prompt to clipboard, opens the user's chosen site, and stores **only one lightweight pending relay descriptor** locally (relay ID, task reference, destination, title and cleaned source URL). It does not inject into the chat form or auto-send.
3. The user pastes, chats, and manually selects or pastes a chosen reply in the popup. **Return to local provider** invokes the optional `relay.return` V1 Native Messaging action only if the installed provider advertises the capability. If there is no compatible host, the user can **Copy return package** and manually hand it to any local tool.
4. A Local Bridge host stores the user-reviewed reply in its existing per-user inbox as source `web-ai-relay` and records `relay_id`, `task_ref`, Web AI service label and original source title/URL. This is a received suggestion, not an executed task or verified code change.

Nothing is sent to ChatGPT, Gemini or other services until the **user personally pastes/submits** the prepared text. No website chat output is automatically scraped. No automatic execution of AI-proposed code or MCP tools occurs.

## B. From an existing Local Bridge inbox task

```sh
local-bridge inbox list
local-bridge relay prepare ctx-<existing-inbox-id>
```

The CLI returns JSON with `task_ref` and `prompt`; the user copies the prompt into any Web AI chat, and later enters the `task_ref` in the Chrome extension if they want the reply associated with this original item. No model API is called. The Chrome extension does **not** need to know an enterprise authentication token.

## C. Native Messaging extension

The existing `host.status` advertises `relay.return` when implemented. The popup-only action accepts:

```json
{
  "protocol_version": 1,
  "request_id": "relay-1",
  "action": "relay.return",
  "payload": {
    "relay_id": "rel-0123456789abcdef0123456789abcdef",
    "task_ref": "ctx-abcdef012345",
    "web_ai": "chatgpt",
    "source": {"title": "Original page title", "url": "https://example.org/page"},
    "reply": {
      "title": "AI response page",
      "url": "https://chatgpt.com",
      "selection": "Proposed fix or summary. NOT YET EXECUTED."
    }
  }
}
```

All text is bounded; URLs reject credentials/query/fragment, identifiers do not become filesystem paths or commands, and only the Chrome popup can issue this action. The Host returns `received:true,stored:true,status:received` with an inbox `task_id`. **It never claims that code was edited or a message was delivered.** A companion AI model may be used later only by a separate explicit user decision.

## Privacy and compatibility

- Minimal metadata only is saved in Chrome; AI replies are **not** stored in extension history. The user may clear the pending handoff. Local Bridge stores only user-chosen returned text in its existing user-owned inbox, until the user removes it.
- Chat service terms and free quotas are not altered or bypassed. The extension does not read cookies, automate logins, scrape AI chat DOM, or perform invisible background requests.
- No new Node/Python/service requirement, paid model API key, cloud relay, database, provider registry or default-install dependency. WeChat remains optional.
- Existing Web SDK read-only availability, Chrome Native Messaging status/share/summarize, user-registered MCP tools and CloudFile production Host/Agent2 are unchanged.
- A real Chrome UI click + native host registration must be verified on the target device; CI/native-process tests prove only code/framing/storage.

**V1 intentionally excludes:** automated Web Chat answer extraction, webpage chat API proxying, MCP tool invocation based solely on model text, prompt/template catalogs, automatic provider switching, quota monitoring, async task queues and 9Router-like unified model routing.
