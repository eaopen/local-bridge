# Personal Agent quick start (Pi first, experimental)

Local Bridge is a **launcher**, not another AI runtime. Users install official
Pi/OpenCode and keep their own provider accounts and data. Default models are
preselected: **no model-choice step** for ordinary users.

## 1. Install once

1. Install the Local Bridge binary for your OS from a successful CI build,
   or compile with Rust 1.90+.
2. Install **Pi**: https://pi.dev/docs/latest/quickstart
3. Optional **OpenCode** for programming: https://opencode.ai/docs/
4. For Pi, get a **Google AI Studio API key** at
   https://aistudio.google.com/api-keys.
   Use an eligible **free-tier project with no paid billing enabled**.
   In Pi, run `/login google` once and paste the key. **No model selection**.
   The credential stays in Pi's own user profile; Local Bridge never reads,
   stores or distributes keys.

## 2. Use

```sh
local-bridge agent doctor
local-bridge agent                  # Pi, the default
local-bridge agent code             # OpenCode, optional for programming
```

| Role | Official CLI | Fixed default | Access |
|---|---|---|---|
| Personal chat | Pi | google/gemini-3.5-flash-lite | Google AI Studio free API tier; own API key |
| Coding | OpenCode | opencode/deepseek-v4.1-flash-free | Current -free catalog; eligibility may vary |

No model picker, automatic paid model fallback or impersonation of OpenCode.
`agent models` is a debugging command, not part of onboarding. Both programs
have an advanced opt-in `--model` override for experts; **that override may
be paid**. Do not use it for the default user experience.

If a model disappears or is unavailable to a region/project, startup reports
the provider error. No silent substitution. Any default change should be
explicitly reviewed, tested and shipped in an update.

### Pi + OpenCode side by side

```sh
local-bridge agent chat --dir /path/to/your/project
local-bridge agent code --dir /path/to/your/project
```

Both use the same **user-selected directory** but retain separate sessions,
context and authentication. They do **not** automatically exchange prompts,
commit code or share long-term personal memories.

Without `--dir`, the launcher uses a dedicated user workspace
`~/.local-bridge/agent-workspace` (Windows: below LOCALAPPDATA), not the
current enterprise directory. A provided `--dir` must already exist.

## Safety and billing boundaries

- Pi runs with **`--no-tools`**: built-in local file-write/shell tools are
  disabled. Review any installed extensions.
- OpenCode uses the built-in **build** agent and asks before Bash/edit/external
  directory actions. CLI extensions and model responses are still external
  software and services.
- **Free API tier is not a universal no-billing guarantee**: eligibility,
  account status, project billing settings and quotas matter. A paid project
  can incur charges. Keep first-use projects unbilled.
- Google may use data submitted to the *free API tier* to improve its products.
  Do not send confidential chats, sensitive personal records or enterprise
  files to the free provider without informed consent.
- Local Bridge downloads no third-party binaries or login credentials.
  It adds no remote AI scripts, HTTP listener or MCP execution tools.
- CloudFile checkout, file edits and versioning remain in Agent2; the bridge
  cannot bypass its user permissions or journal.

The next major UX milestone is a small GUI/tray + official installation flow,
**not** a new model selector or a second AI engine.

Official references:
[Pi CLI](https://pi.dev/docs/latest/cli) ·
[Pi provider setup](https://pi.dev/docs/latest/providers) ·
[Google Gemini API pricing](https://ai.google.dev/gemini-api/docs/pricing) ·
[OpenCode](https://opencode.ai/docs/).
