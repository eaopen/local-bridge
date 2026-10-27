# MCP stdio integration (0.1.0)

The bridge is **not an AI model, orchestration loop or remote MCP gateway**. Its `local-bridge mcp` command is a small read-only MCP Server for user-approved local clients. Separately, the `local-bridge provider ...` CLI now operates as an optional MCP Client for installed providers; those tools are **not** automatically exposed through the MCP Server.

Example configuration (exact integration keys differ between MCP clients):

```json
{
  "mcpServers": {
    "local-bridge": {
      "command": "/ABSOLUTE/PATH/TO/local-bridge",
      "args": ["mcp"],
      "env": {
        "LOCAL_BRIDGE_CLOUDFILE_HOST": "/ABSOLUTE/PATH/TO/cloudfile-launcher"
      }
    }
  }
}
```

Supported JSON-RPC methods: `initialize`, `ping`, `tools/list`, `tools/call`; MCP transport: `stdio`; no network listener.

Available 0.1.0 tools:

| Tool | Effect | Limits |
| --- | --- | --- |
| `bridge.status` | Returns version and capability IDs | Read-only, no local path in output |
| `cloudfile.status` | Calls the locally configured CloudFile 2.3 executable `--component` | Read-only; sanitizes output; local user config only |

**Not implemented by this MCP Server:** `file.open`, `file.edit`, Checkout/Checkin, arbitrary application invocation, remote Channel fetching, cloud AI relay, or proxying registered provider tools to AI clients. These require additional typed operations, local authorization, and CloudFile IPC integration. Do not advertise them to AI clients yet.

Example direct smoke test:

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | local-bridge mcp
```

AI clients must be approved at the user or enterprise policy layer. Local stdio is not a substitute for file-specific authorization. All future sensitive operations must confirm that the user intended the action.

## Separate installed MCP Client path

`local-bridge provider add <id> <absolute-executable> [fixed-args...]` registers an installed user-owned MCP stdio process for **explicit local CLI** invocations; `provider tools/call` uses real `initialize` / `tools/list` / `tools/call`. The first independent example is [WeChat Provider](LOCAL_MCP_PROVIDER_V1.md); see the [actual no-account interoperability test](WECHAT_PROVIDER_MCP_SMOKE.md). The provider's chat/RPA/index/send state remains with the provider. `wechat_send_message` and other writes are **not** advertised to this Bridge MCP Server or webpages. Real WeChat GUI and delivery remain unverified.

## Unified Profile / task / Pi plan

[Capability / MCP / Pi V1](CAPABILITY_AND_MCP_V1.md) defines the **planned** unified tool contract. This 0.1.0 MCP server still exposes **only** the two read-only tools listed above. Neither a declared ApplicationProfile channel nor a Pi process automatically enables future file or enterprise write tools.
