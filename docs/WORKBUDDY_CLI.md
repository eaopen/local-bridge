# Bridge → WorkBuddy Local Assistant

Implemented CLI transport; live OAuth/API acceptance remains pending. A separate desktop-account WBIPC + bundled CLI/ACP experiment passed real text/file tasks on 2026-10-28 (5.9.3). See [validation results](AGENT_PROVIDER_VALIDATION.md). That success does not verify this OAuth adapter.

Supply `LOCAL_BRIDGE_WORKBUDDY_ACCESS_TOKEN` through your existing trusted application's credential delivery mechanism. Do not paste tokens into commands, repository files, or logs. OAuth application registration and token issuance/refresh stay with the authorized application; Bridge does not embed a client secret or extract desktop credentials.

Required scopes: `user.localassistant.readable` for status/history and preflight; `user.localassistant.invokable` for submission. Server responses enforce scopes; successful status cannot establish invoke authorization.

```sh
local-bridge agent workbuddy status
printf '%s' 'Only reply WB_CHECK=42; do not use tools.' | local-bridge agent workbuddy send -
local-bridge agent workbuddy messages <returned-message-id>
```

`send` checks online status, submits once and returns `accepted` plus `message_id`. It does not enforce a read-only sandbox: WorkBuddy retains its own tool permissions. Only submit tasks you intend WorkBuddy to execute.

`messages` retrieves the documented incremental history after the supplied ID. History is account-wide and may contain concurrent conversations; it is not a task-correlated result. The official Local Assistant contract has no completion event, so the adapter returns `status: unknown` and `completion_verified: false`, even when assistant text exists. Consult WorkBuddy to verify completion. Do not resend after timeouts or uncertain submissions. No retries, provider failover, permanent chat storage, or automatic approvals are introduced.

Transport uses the fixed official HTTPS origin, disables redirects, limits responses to 4 MiB and requests to 30 seconds. Error output does not echo upstream bodies or bearer tokens. Missing/expired OAuth or insufficient scopes must be resolved with the original authorizing application.

This is an explicit local CLI capability. Browser/Native Host and MCP do not expose WorkBuddy execution. WorkBuddy → Bridge MCP registration remains a separate integration.

Contract: https://open.workbuddy.cn/docs/openapi (Local Assistant section, checked 2026-10-28).
