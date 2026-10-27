# Architecture — Local Bridge

## Purpose

Local Bridge is **not** another WorkBuddy/Claude/Copilot, an MCP gateway fleet, RPA suite or a SeaDrive replacement. It supplies deterministic desktop capabilities to authorized enterprise Web and AI clients.

## Three-party boundary

Any enterprise application publishes a **generic** ApplicationProfile and owns its business objects, authentication and business permissions. Local Bridge must not depend on the name, data model or private code of the application. CloudFile and Bridge may know each other as mutually integrated capabilities: Bridge routes a file task; CloudFile Agent owns its authorized file execution, Journal and checkout state. No existing application must migrate or change its production behavior to develop Bridge in parallel.

Target contracts: [ApplicationProfile](APPLICATION_PROFILE_V1.md), [Tasks](BRIDGE_TASK_V1.md), [Capability/MCP](CAPABILITY_AND_MCP_V1.md), [nonbreaking migration](CLOUDFILE_TO_BRIDGE_MIGRATION.md).

## Components

- **Rust Runtime**: one local capability registry, task gate, trusted clients, structured results, audit interfaces. Current repo starts with a small CLI; task supervisor and authorization are planned.
- **MCP adapter**: stdio, no open network port by default. The adapter owns no file authorization. It invokes a local trusted runtime service, not shell text from the model.
- **CloudFile application**: first adapter targets the **existing** cloudfile-local-agent2. The file module retains native Seafile protocol, authorization, checkout guard, Journal, CDC, upload receipt and rollback behavior. We do not fork it.
- **Application Profiles / Enterprise Channels**: the enterprise server supplies declarative labels, Web route, requested capability IDs and UI metadata. Runtime only shows entries backed by trusted modules. It does not download programs.
- **AI**: external MCP clients are optional. Core must function with no installed AI.

## Integration boundaries

```text
enterprise application API + CloudFile Hub/Server: existing authorization
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
