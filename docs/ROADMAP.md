# Roadmap (proposal, not completed work)

**0.1.0 — repository baseline**
- Rust workspace + local MCP stdio read-only endpoint.
- Validated enterprise Channel schema (offline only).
- CloudFile Agent2 identity adapter, no duplicated file-edit engine.
- Cross-platform CI for Rust.

**0.2 — CloudFile application**
- Reuse CloudFile 2.3 durable file engine via authenticated local IPC.
- Read task list and open user-authorized files; no raw paths to AI.
- Real Office/CAD on Windows standard user, macOS QA.
- Write/Checkout/Checkin remain user-confirmed and continue using Agent2.

**0.3 — Enterprise Channel MVP**
- Trusted provider enrollment; provider-bound signed/authenticated config fetch.
- Dynamic enterprise Web entries and built-in capability visibility.
- One Etech proof of concept for an approved tool with typed inputs/outputs.

**Later only if needed**
- Safe remote AI relay with device/user/task binding.
- Optional signed native extension distribution.
- Linux desktop packaging and enterprise deployment policy.
- Device/printing/CAD automation based on real customer use cases.

Out of scope: competing AI model/chat UI, generalized RPA, filesystem driver,
Seafile sync client, plugin marketplace, arbitrary remote scripts.
