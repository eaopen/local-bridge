# Local Bridge — Agents / Contributors

## The first principle: make the user's tools collaborate

**Local Bridge is a lightweight local capability and task collaboration platform for personal and enterprise users.** Its value is eliminating copy/paste, downloads/uploads, repeat logins and app switching — not creating another platform to manage.

Before any design, ask: (1) Which real user pain is solved? (2) Which actions disappear? (3) Which existing tools are reused? (4) Does the change create a new obstacle?

### Confirmed product decisions (2026-10-27)

- **Default mixed, optional isolation:** one OS user's enterprise ApplicationProfiles and lightweight local PersonalProfile coexist; share installed, already-authorized tools/Agent/MCP/communication capabilities and one task view. Do not add blanket cross-profile approval gates.
- **Authentication is application-owned:** reuse Etech/CloudFile/Authentik or other existing SSO. Chrome Web uses existing login for scope-bound, short-lived action tickets. Never extract browser cookies/passwords for Bridge/Agents.
- **Files remain CloudFile-owned:** Agent2/Hubs/Server own versions, Guard, Journal, Checkout/Commit; never copy execution logic or take over an existing Windows Host/unfinished task.
- **Communication adapters are open:** a user-approved official/third-party installed connector may expose message read/subscribe/send. WeChat/Soul are examples, not guaranteed official APIs. Do not build private-protocol bypasses or assume consent overrides platform rules.
- **Boundary proportional to the task:** check actual caller, installed capability, connector grant and underlying service authorization. Chat/AI/web content itself is not permission to run arbitrary commands. Do not build new enterprise DLP/SSO/policy engines.
- **Truthful project status:** 0.1.0 has only read-only MCP status and offline Profile structure validation. Do not label planned Agent runs, communication connectors, Profile registration or file IPC as shipped.

Prefer one real end-to-end slice and essential low-cost regression over a huge plugin framework, task DB, new config surface or extra approval steps. DEV/PROD Agent Host, credentials, Journal and working copies remain separate to prevent wrong file operations.

Read [product value/use cases](docs/PRODUCT_POSITIONING_AND_USE_CASES.md), [Profile/SSO/mixed workspace](docs/PROFILE_AUTH_AND_COLLABORATION.md), [roadmap](docs/ROADMAP.md), [ApplicationProfile](docs/APPLICATION_PROFILE_V1.md), [Agent + communications](docs/AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md) and [CloudFile migration](docs/CLOUDFILE_TO_BRIDGE_MIGRATION.md) before modifying behavior.
