# Architecture — Local Bridge

## Purpose

Local Bridge is a **lightweight personal + enterprise local capability and task collaboration bridge**. Its job is removing the user's cross-app friction by connecting tools they already use. It is **not** another WorkBuddy/Claude/Copilot, enterprise DLP, identity provider, MCP gateway fleet, RPA suite or SeaDrive replacement.

## Three-party boundary

Any hosted enterprise or personal Web application publishes a **generic** ApplicationProfile and owns its account, objects, authentication and business permissions. A local user's PersonalProfile is a planned light on-device workspace concept, not a replacement for the ApplicationProfile V1 hosted schema. Local Bridge must not depend on the name, data model or private code of the application. For the **second independent product stack**, Bridge may route a file task only to its separately installed `cloudfile-bridge-provider`; that Provider owns its own authorized file execution, Journal and checkout state. The first CloudFile Agent2 + CloudFile Chrome stack remains a separate installed product; it is **not** an implicit Bridge Provider, local task store, or fallback. No existing application must migrate or change its production behavior.

Target contracts: [ApplicationProfile](APPLICATION_PROFILE_V1.md), [Tasks](BRIDGE_TASK_V1.md), [Capability/MCP](CAPABILITY_AND_MCP_V1.md), [nonbreaking migration](CLOUDFILE_TO_BRIDGE_MIGRATION.md).


## Mixed workspace and upstream SSO

- **One runtime, mixed by default:** the same computer can edit enterprise CloudFile documents and run a personal WeChat assistant. Profiles identify source/login/task ownership; they do not automatically prohibit use of another Profile's already-authorized tools. Optional isolated workspace restricts Bridge-local routing, not the OS.
- **Existing app authentication wins:** Chrome Web already logged into Etech/CloudFile/another app → app backend grants a short-lived scope-bound action → one explicitly selected product's Chrome extension/Native Host → its own file Provider/Connector performs that action. The original CloudFile Chrome continues routing only to its original Agent2, not through Bridge. CloudFile keeps its V2 file tickets, Guard and Journal.
- **No second SSO or DLP:** Bridge never extracts browser cookies or replicates existing IdP/object permissions. It validates its own caller, registered typed action and user-enabled Connector. Model/chat/web text alone cannot become permission for arbitrary OS commands.
- **Dev/Prod runtime identities remain distinct:** Host, working copies, Journal and update trust must not overwrite each other for operational correctness. Personal/enterprise sharing is a separate concept.

See [product user scenarios](PRODUCT_POSITIONING_AND_USE_CASES.md) and [Profile/SSO/mixed collaboration](PROFILE_AUTH_AND_COLLABORATION.md).

## Components

- **Rust Runtime**: one local capability registry, task gate, trusted clients, structured results, audit interfaces. Current repo starts with a small CLI; task supervisor and authorization are planned.
- **MCP adapter**: stdio, no open network port by default. The adapter owns no file authorization. It invokes a local trusted runtime service, not shell text from the model.
- **Second-stack CloudFile Provider**: an independent `cloudfile-bridge-provider` executable is built from the Agent2 Provider branch, sharing file-Core source/semantics but **not the first stack's installed process, Chrome Host, private Journal or workspace**. It keeps native Seafile protocol, authorization, checkout guard, Journal and upload receipts in its own scope. File writes are not yet implemented through Bridge.
- **Profiles / Channels**: hosted applications declare Web entry and requested capabilities; a planned local PersonalProfile exposes user-selected installed tools. Runtime shows installed/authorized capabilities; it does not download executable programs from Profile text.
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
   Independent cloudfile-bridge-provider (optional)
                 |
     Local document / Office / CAD
```

**Current:** `cloudfile.status` is a legacy, explicitly configured read-only component diagnostic and **not** the second stack's standard file provider. The new `cloudfile-bridge-provider` has its own MCP stdio entry, separate user data root and two read-only tools; no first-stack task/Journal data is imported, and CloudFile file editing remains unavailable in the second stack until separately verified.

**Next:** progressively implement scoped, independently owned file tasks and durable IPC in the second stack's Provider: first its own task summary, then view/edit/exclusive. Do not call the original CloudFile Host or share its private data. Never bypass CloudFile Server authorization and the Provider's own Journal with Bridge-side writes. The first stack continues as-is until a future explicitly approved merge upgrade.

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
