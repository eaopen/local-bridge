# Security and Trust Rules

- **No arbitrary shell/EXE from enterprise manifest, Web page or AI prompt.**
- All MCP tools are explicitly implemented, read-only at 0.1.0, bounded in input size. Unknown methods/capabilities are rejected.
- CloudFile adapter executable location is set by the local user as an absolute path and must match the exact known filename. The AI cannot pass command names or process arguments.
- Only non-sensitive component metadata can be returned to MCP; never expose local paths, tokens, device keys or enterprise cookies.
- A Channel manifest has strict type, bounded cardinality/length, expiry, validated canonical HTTP(S) origin and same-origin relative Web paths. Validation is **not** authentication.
- Until a trusted fetch/authentication mechanism is implemented, configs are supplied via a local file solely for validation. Do NOT turn a remotely received unsigned manifest into executable capabilities. HTTP network attackers can modify unsigned configurations.
- Browser authorization and CloudFile file ACL enforcement remain in CloudFile/Etech; no permission is inferred from merely seeing a Channel.
- Native file-editing and version publication must call the existing Agent2 module, with its short-lived grants, Guard and durable Journal, not create a second upload engine.
- MCP stdio does not authenticate arbitrary local client processes by itself. Only connect it to user-approved AI apps and never grant powerful tools on the strength of this transport alone.
- Avoid default localhost web listeners, unrestricted filesystem access, remote code download, secrets in logs, silent Checkout release and auto-triggered write actions.

- Optional Personal Agent launch is a **local user's explicit CLI action**, not
  an MCP/Channel ability. The Bridge never accepts AI-provided executable names
  or command options, never installs a model CLI automatically, and never opens
  an HTTP listener. It starts only `pi` and `opencode` from the user's PATH.
- Default Agent workspace is separate from the current enterprise directory.
  OpenCode uses the built-in plan agent, with shell/edit/external access requiring
  user confirmation in runtime config; Pi uses `--no-tools`. These are not
  guarantees that a third-party CLI or installed extension cannot access data.
  Use only trusted, up-to-date binaries and review third-party plugins.
- **Pi is the default chat entry**, pinned to Google's Gemini 3.5 Flash-Lite
  API model. Its free tier requires an eligible project and user API key;
  account billing, region and quotas can change costs or access. Google's
  free tier may use conversation inputs/outputs to improve its products.
  No paid-model fallback. Do not send sensitive or enterprise data without
  informed consent.
- OpenCode is a separate, optional coding mode pinned to a known `-free`
  model, rather than a random discovered default. The `-free` catalog listing
  is not a billing, quota or uptime guarantee.
  The Bridge never falls back silently to a paid model or borrows credentials
  from a different client's private login state.
- The optional `--dir` path is provided by the local user only. Data shared in
  prompts may be sent to external providers; enterprise files require separately
  authorized user action.

Security reports: use the repository maintainers' private contact method; do not post credentials or exploit payloads in a public issue.
