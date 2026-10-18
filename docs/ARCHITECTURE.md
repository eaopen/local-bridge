# Architecture — Local Bridge

## Purpose

Local Bridge is **not** another WorkBuddy/Claude/Copilot, an MCP gateway fleet, RPA suite or a SeaDrive replacement. It supplies deterministic desktop capabilities to authorized enterprise Web and AI clients.

## Components

- **Rust Runtime**: one local capability registry, task gate, trusted clients, structured results, audit interfaces. Current repo starts with a small CLI; task supervisor and authorization are planned.
- **MCP adapter**: stdio, no open network port by default. The adapter owns no file authorization. It invokes a local trusted runtime service, not shell text from the model.
- **CloudFile application**: first adapter targets the **existing** cloudfile-local-agent2. The file module retains native Seafile protocol, authorization, checkout guard, Journal, CDC, upload receipt and rollback behavior. We do not fork it.
- **Enterprise Channels**: the enterprise server supplies declarative labels, Web route, requested capability IDs and UI metadata. Runtime only shows entries backed by trusted modules. It does not download programs.
- **AI**: external MCP clients are optional. Core must function with no installed AI.

## Personal Agent launcher (optional)

A thin **user-invoked** Rust subprocess launcher starts the official Pi or OpenCode
CLI in a user-controlled workspace. This is separate from the trust boundary
of CloudFile and enterprise Channels; there is **no AI execution capability in
MCP**. Model access and authentication remain in the vendor CLIs. No remote
script execution, shell delegation through manifests or background server is added.
Pi chat and OpenCode coding can share an explicit user-selected folder but not
credentials, sessions, memories or privileges. The chosen free-tier models
require provider authorization and may be restricted by quotas, region or
project billing settings. This launcher is not a full agent/chat runtime.

See [Personal Agent starter](PERSONAL_AGENT.md) for exact commands.

## Integration boundaries

```text
enterprise CloudFile/Etech API: identity, roles, object permissions, versions
                       |
     native browser/short-lived authorization + eventual enterprise channel config
                       |
            [Local Bridge Runtime]
                 /          \
      MCP stdio adapter   Web/native adapter (future)
                 |
         CloudFile adapter
                 |
   Installed cloudfile-local-agent2 (existing)
                 |
     Local document / Office / CAD
```

**Current:** `cloudfile.status` runs the locally configured CloudFile executable with `--component`, checks the expected family and returns a *redacted, read-only subset* of version metadata. CloudFile editing remains in its existing repository and is NOT exposed from this bridge until a verified local IPC contract exists.

**Next:** implement a mutually authenticated, user-owned IPC binding for file tasks. Map only narrow operations such as list task / open authorized file / request checkout / confirm commit. Never bypass the CloudFile authorization and Journal with bridge-side writes.

## Execution policy (future, not implemented)

1. Identify caller and enterprise provider; bind request to local user and trusted origin.
2. Verify capability ID and typed arguments; reject unknown operations.
3. For file access, use an already-authorized CloudFile task or a user-selected file; do not accept arbitrary remote absolute paths.
4. Require user confirmation for writes, checkin or launching a custom application.
5. Record task state; the owning module handles receipts, rollback, leases and conflict resolution.

## HTTP and HTTPS

Both are supported for user-authorized enterprise applications, including private IP/hostname deployments. The **MCP stdio adapter never listens on TCP**. Cross-device MCP transport (if ever needed) belongs behind a separately authenticated enterprise relay; no endpoint is currently implemented. HTTP data has no transport confidentiality; especially sensitive tokens and enterprise Channel downloads must have an additional trust mechanism or use HTTPS.

## Deployment

User-owned executable, no admin, no forced background service, no WebView2 for CLI/MCP; Windows/macOS first, Linux core feasible. Portable first; signed installation and automatic update reuse principles established in Agent2.3, subject to own testing. No promise of Windows policy bypass.
