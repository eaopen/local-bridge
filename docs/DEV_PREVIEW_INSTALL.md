# Local Bridge developer preview (unsigned)

This build is a **temporary, unsigned development preview**, not a Store release or production update. It does not modify or replace CloudFile Agent2, the original CloudFile Host/extension, or file Journal/Guard.

Download the relevant artifact from [Actions → Unsigned Local Bridge developer preview](https://github.com/eaopen/local-bridge/actions/workflows/dev-preview.yml). Verify the selected commit and file hash before executing.

- `local-bridge-dev-macos-arm64`: tar.gz with CLI + native host in `bin/`, and optional `Local Bridge Desktop Dev.app`.
- `local-bridge-dev-windows-x64`: ZIP with CLI + native host in `bin/`, and an **unsigned** Tauri NSIS installer.

Keep `local-bridge` and `local-bridge-native-host` side-by-side. The desktop manager is independent and not necessary for the CLI/Host.

## Isolated CLI smoke

Use a new test user-home/profile, not your regular CloudFile/Chrome profile.

```sh
./bin/local-bridge status
./bin/local-bridge self-test
./bin/local-bridge native-host status
./bin/local-bridge agent-preferences get
./bin/local-bridge tasks list
```

The desktop app must be launched manually and tested in a separate DEV account/home. Verify Overview → Tasks → Settings, save Agent selection, close/reopen, then read back using the same CLI settings file. Do **not** install a user-wide background daemon or alter OS startup in this preview.

## Real Chrome DEV E2E

1. Create an isolated Chrome profile and load the separate [Local Tool Bridge](https://github.com/eaopen/local-bridge-chrome-extension) unpacked extension. Obtain the **real** 32-letter extension ID.
2. Register only `com.eaopen.browser_bridge` via `local-bridge native-host install <real-test-extension-id>`. Never substitute the original CloudFile Host ID, and do not force-replace a preexisting manifest.
3. Use a harmless test page and explicitly approved exact-Origin grants. Validate real Chrome Native Messaging: `host.status` → user-triggered `context.share` → `inbox list/read`, then one registered read-only MCP `*_status` tool and its returned result.
4. Only with an already authenticated and locally authorized Agent, test a short `runAgent/readAgent` through the real browser. An accepted task is not completed; follow final status, retry the same clientKey after transport breaks, and check revocation.
5. Clean up the isolated test Chrome profile and temporary grants. If no real model, Provider or browser permission is available, record **not verified**, not success.

The original CloudFile data is still owned by Agent2. The optional [Agent2 Provider Draft #26](https://github.com/eaopen/cloudfile-local-agent2/pull/26) is read-only and has **not** merged into Agent2 dev. Test it against the **same** original Agent2 data root; an empty separate root must not be mistaken for zero active file tasks.

## Release limits

The artifact is unsigned and unnotarized (may be refused by Gatekeeper/SmartScreen). Do not disable device-wide security controls, install as production, or claim real GUI/Chrome E2E based on compilation alone. Formal signing, updater and rollout are separate milestones.
