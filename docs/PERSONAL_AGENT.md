# Personal Agent — quick start (experimental)

Local Bridge can launch two **user-installed, official** Agent CLIs. This is a local,
opt-in convenience layer, **not** a hosted Personal Agent, social account, remote
shell, or MCP tool.

## Before first use

1. Download the portable `local-bridge` binary from a successful CI build for
   your operating system, or run `cargo build -p local-bridge --release --locked`
   with Rust 1.90+. Windows users can run the .exe from their user directory
   without administrator privileges.
2. Install **OpenCode** via the official guide:
   https://opencode.ai/docs/ . No bridge-managed installer or script download.
3. Open a *new terminal* so the CLI is on PATH.

```sh
local-bridge agent doctor
local-bridge agent models
local-bridge agent
```

`agent` opens the **official OpenCode terminal UI** in a private, separate
workspace (`~/.local-bridge/agent-workspace`; on Windows, under LOCALAPPDATA).
It selects a currently listed `opencode/*-free` model. If OpenCode has
removed the model, authentication or free quota has changed, the upstream CLI
may still reject the request. We do not bypass its login or free-tier restrictions.

To choose an existing free model explicitly:

```sh
local-bridge agent opencode --model opencode/mimo-v2.6-flash-free
```

That is **an example model ID**, not a promise that it is currently available.
Run `local-bridge agent models` first. Only model IDs returned by the installed
OpenCode catalog and ending with `-free` are accepted by this starter.

### Pi Agent (optional)

Install Pi via https://pi.dev/ (official installer). It is **not** automatically
installed and can be used without OpenCode:

```sh
local-bridge agent pi
local-bridge agent pi --model opencode/mimo-v2.6-flash-free
```

The optional `--model` selection uses **Pi's own official provider and account
authentication**. It requires a supported /login or API key as appropriate:
a free model *price* is not proof that anonymous third-party API access works.
Pi is launched with `--no-tools` so it can chat without running local commands.
Depending on your Pi account/model, requests may be billable. Verify pricing in Pi.

For personal files, choose an explicit directory:

```sh
local-bridge agent opencode --dir /path/to/your/project
local-bridge agent pi --dir /path/to/your/project
```

A provided `--dir` must already exist. By default the launcher creates a
separate user-owned workspace and does not inspect the current enterprise work
directory. OpenCode uses the built-in `plan` Agent and asks the user before
shell/file writes; Pi has tools disabled completely. Both may send the messages
and user-selected content to their respective provider; do not put secrets in
prompts or select enterprise folders without authorization.

## What this MVP does **not** do

- Does not install/update Pi or OpenCode, reuse OpenCode internal private APIs,
  impersonate its client, share its credential files, or automatically supply keys.
- Does not expose AI tool execution through the Local Bridge MCP or Channel manifest.
  Existing `bridge.status` and `cloudfile.status` stay read-only.
- Does not add a localhost HTTP listener, UI, tray icon, autostart, shared
  multi-user state, chat memory migration, or mobile/WeChat integration.
- Does not write CloudFile documents: those remain in CloudFile Agent2.

Future work should add a reviewed user-approval task interface for native tools
*before* integrating full Pi/OpenCode tool execution. No silent remote execution.

Sources: [OpenCode CLI](https://docs.opencode.ai/docs/cli/),
[OpenCode model/pricing listing](https://docs.opencode.ai/docs/zen/),
[Pi CLI and tool flags](https://pi.dev/docs/latest/cli),
[Pi providers/models](https://pi.dev/docs/latest/models).
