# MCP stdio integration (0.1.0)

The bridge is **not an AI model, orchestration loop or remote MCP gateway**. It is a local MCP tool provider for an AI client already authorized by the user.

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

**Not implemented:** `file.open`, `file.edit`, Checkout/Checkin, arbitrary application invocation, remote Channel fetching, cloud AI relay, generic MCP proxy. These require additional typed operations, local authorization, and CloudFile IPC integration. Do not advertise them to AI clients yet.

Example direct smoke test:

```sh
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | local-bridge mcp
```

AI clients must be approved at the user or enterprise policy layer. Local stdio is not a substitute for file-specific authorization. All future sensitive operations must confirm that the user intended the action.
