# Local Bridge

**面向个人与企业的轻量级本地能力连接与任务协作平台。**

*Connect the apps, files, AI agents and communication tools people already use — on their own computer.*

**最高产品原则：让用户更方便协作。** 将本机原本割裂的平台连接起来，解决复制粘贴、重复登录、导入导出、工具切换和任务难以追踪等实际困扰，而不是让用户维护新的复杂系统。

[个人/企业/混合使用场景](docs/PRODUCT_POSITIONING_AND_USE_CASES.md) · [混合 Profile 与 SSO](docs/PROFILE_AUTH_AND_COLLABORATION.md) · [Architecture](docs/ARCHITECTURE.md) · [Roadmap](docs/ROADMAP.md) · [Security](docs/SECURITY.md)

> **当前实现：0.1.0 核心及后续最小接线。** 已有只读 MCP Server、离线 Profile 校验、本机显式 Codex CLI run、Chrome Native Host/手动网页收件箱，以及用户注册的 MCP Client（已与独立 WeChat Provider 完成不涉及账号的协议联动）。**尚未交付**真实 Chrome/微信桌面实机验收、可信企业 SSO/Web 动作、PersonalProfile 运行时、双向通讯自动订阅、通用 BridgeTask 运行时或 CloudFile 文件编辑接管。现有文件编辑仍由 [CloudFile Agent2](https://github.com/eaopen/cloudfile-local-agent2) 负责。

## 为什么需要 Local Bridge

同一台电脑上，企业 Web、个人微信等通讯、CloudFile、Office/CAD 和已有 Codex/WorkBuddy/Pi 往往各自工作，用户要反复复制消息、下载文件、切窗口和重新登录。

Local Bridge 提供**最小的本机能力连接层**：
- **企业：**已登录的 Web 可经原应用授权调用 CloudFile、本机专业软件、MCP 和 Agent。
- **个人：**复用已安装的 AI/MCP、个人文件和获用户许可的通讯 Connector；微信、Soul 等是否可读/订阅/发送取决于实际第三方接入能力。
- **混合：**同一工作电脑同时编辑企业文件和运行个人聊天助手，**默认混合、按需隔离**，不强制切换 Profile 或增加额外的跨 Profile 审批。
- **渐进兼容：**不安装 Bridge 时现有企业 Web、CloudFile 文件三模式和本地工具仍正常使用。

Profile 用于来源、账号上下文与任务归属，**不默认作为个人/企业数据墙**。已有应用负责 SSO、业务权限与文件版本；Bridge 只承担自身可信调用、能力分发与任务交接，不建设新 DLP 或企业认证中心。

**AI 模型从哪里来，不由 Bridge 统一解决。** 企业 Web/服务方已有 AI API 时，由企业应用服务端管理模型接入、账号授权、计费与实际推理；Bridge 只提供经授权的本机 MCP/Agent 能力和结果交接。个人用户优先复用已登录 Codex、WorkBuddy；Pi 只在本人已经配置可用模型时按需调用。没有 API 或 Agent 时，现有 Chrome Web AI Relay 可让用户手动使用已登录网页聊天。**不建设 Bridge 模型 API Key 配置界面、免费额度路由或类似 9Router 的网关。**

**可选 Chrome → 本地 Codex 摘要（需主动点击）：** 本机 Native Host 可接收已安装的独立 Local Tool Bridge Chrome 扩展提交的 `context.summarize`，调用用户已有、已登录的 Codex CLI，并将结果回传 Popup；浏览器网页无法直接调用，Bridge 不预装 Codex，扩展无本机 Provider 时仍能独立采集/复制。此功能是可选的显式用户动作，Chrome/macOS/Windows 实机联动仍需验收，详见 [Native Host V1](docs/CHROME_NATIVE_HOST_V1.md)。

**可选 Web AI 手动接力（非模型路由）：** Local Tool Bridge 可以将用户选择的网页上下文复制给已有 ChatGPT/Gemini/Claude 等 Web Chat，由用户在浏览器中自行粘贴和对话；收到回复后再手动导入同一 Local Bridge 本机收件箱，并关联源任务引用。CLI 也提供 `local-bridge relay prepare <inbox-id>` 生成可复制的接力提示。没有网页登录自动化、网页额度调度、API Token、远程模型网关或自动 MCP 指令执行，详见 [Web AI Relay V1](docs/WEB_AI_RELAY_V1.md)。

**企业模型供给 Pi（无需终端用户配置 API）：** 如果当前已登录企业应用能通过**受信调用上下文**提供 OpenAI-compatible Chat Completions URL、模型 ID 和短期访问票据，Local Bridge 可用 `agent pi enterprise <prompt|->` 为本次 Pi 进程创建隔离模型配置。Pi 调用企业 API，用户不需要配置 Pi 模型或重复登录；企业仍控制账号、请求授权与计费，Bridge 不持久化 Token、不接管企业模型服务。当前交付的是**进程级接入**；从真实企业 Web 自动获取/验证短期票据的通道仍需完成可信 Web ↔ Local 接线。参见 [企业 AI 给 Pi 供模型](docs/PI_ENTERPRISE_AI.md)。

**可选 Pi 模型 API 备选：** 现在可以由用户在 Pi 中直接配置 DeepSeek、Gemini 等已有/低成本 API Key 后，使用 `local-bridge agent pi <provider> <model> <prompt|->` 运行一次无工具副作用的文本任务。Bridge 不保存或代理模型密钥，不默认安装 Pi、不切换免费额度、不重复建模型路由；没有 Pi 凭据时返回错误，仍可手工使用原有 [Web AI Relay](docs/WEB_AI_RELAY_V1.md)。参见 [Pi 最小可用方案](docs/PI_MINIMAL_PROVIDER.md)。

**可选 MCP Provider 示例（非默认功能）：** Local Bridge 的通用 MCP Client 可以连接多种由用户自行安装和注册的本地 Provider，[WeChat Provider](https://github.com/eaopen/wechat-provider) 只是其中一个示例。**不安装微信、不安装 Python/微信依赖、不注册任何通讯 Provider，Local Bridge 也应独立构建、启动、更新，并正常提供其余功能。** 只有用户明确安装和注册微信 Provider 后，才尝试使用其读写工具；消息、账号与发送状态仍由独立 Provider 管理。参见 [可选 Provider 接入](docs/LOCAL_MCP_PROVIDER_V1.md)。真实微信 4.x 实机兼容尚未验收。

## 通用 Profile 与长期演进合同

**一个 Runtime 同时支持个人和企业。** 有服务端的企业或个人 Web 应用使用 ApplicationProfile V1；纯本机个人工具使用规划中的轻量 PersonalProfile/默认个人工作空间（尚非已交付的新 Schema）。两者共享通用 Capability 和 BridgeTask，默认混合；Bridge 核心仍不内建企业应用业务规则。

- [ApplicationProfile V1](docs/APPLICATION_PROFILE_V1.md)：应用配置、可信来源、Dev/Prod、更新及稳定身份
- [BridgeTask V1](docs/BRIDGE_TASK_V1.md)：统一文件/企业/AI 任务中心合同
- [Capability / MCP V1](docs/CAPABILITY_AND_MCP_V1.md)：通用工具与 MCP 权限
- [Agent Providers / Local Communications V1](docs/AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)：WorkBuddy / Codex / Pi 本地选择、可选通讯连接器
- [Codex / WorkBuddy Agent 实机验证](docs/AGENT_PROVIDER_VALIDATION.md)：分别记录安装、授权、官方接口、真实任务及未完成项
- [CloudFile → Bridge 迁移](docs/CLOUDFILE_TO_BRIDGE_MIGRATION.md)：并行开发不接管或破坏现有文件能力
- [Chrome Native Messaging V1](docs/CHROME_NATIVE_HOST_V1.md)：可选 Local Tool Bridge 原生 Host、安装和本机收件箱
- [个人/企业/混合场景与诉求](docs/PRODUCT_POSITIONING_AND_USE_CASES.md)：按真实用户任务组织需求与验收
- [混合工作空间、企业 SSO 与 Chrome 授权](docs/PROFILE_AUTH_AND_COLLABORATION.md)：已有认证不重复实现

```sh
cargo run -p local-bridge -- validate-profile examples/application-profile.json
```

`validate-profile` **仅验证 JSON 结构**，返回 `trusted=false` / `activated=false`；不会下载配置或注册 Host。现有 `validate-channel`、MCP 的 `bridge.status` 和 `cloudfile.status` 完全保持原样。

## 结构

```text
Personal Agent / MCP       Enterprise Web / CloudFile / Personal Apps
          |                          |
          +---- Local Bridge --------+
               | Rust Runtime
               | Auth / capability policy (planned)
               | Channels (declarative, no arbitrary code)
               +-- CloudFile adapter (first)
               |     -> existing cloudfile-local-agent2
               +-- Approved enterprise tools (future)
```

当前工作区：

| Component | Purpose | Current implementation |
| --- | --- | --- |
| `crates/local-bridge-core` | Channel schema, origin validation, capability allowlist | Tested |
| `crates/local-bridge` | CLI、只读 MCP Server、本机 Codex、Native Host/Inbox、用户注册的 MCP Client | 基础能力已实现；真实 GUI/微信与 Web/SSO 联动仍未验收 |
| CloudFile adapter | Connects installed Agent2 via a user-configured executable | Read-only `--component` probe |
| Enterprise Channels | Server describes allowed navigation and built-in capabilities | Offline manifest validation only |
| File editing / Checkout | CloudFile’s actual file workflows | Continue in the existing Agent2 repo; **not yet bridged into MCP** |

## 构建及本地测试

Requires Rust **1.90+**. No Node.js, Python, Docker or model runtime required at runtime.

```bash
cargo test --workspace
cargo run -p local-bridge -- status
cargo run -p local-bridge -- mcp
# Optional user-registered independent stdio MCP provider:
# cargo run -p local-bridge -- provider add wechat /absolute/path/to/wechat-provider serve
# cargo run -p local-bridge -- provider tools wechat
# Optional independently installed Chrome Native Messaging provider:
# cargo build --workspace --bins
# cargo run -p local-bridge -- native-host install <extension-id>
# cargo run -p local-bridge -- inbox list
# Optional explicit local Agent task (requires installed + logged-in Codex CLI):
cargo run -p local-bridge -- agent codex "Summarize the current working directory"
# Or hand selected/piped text from a user-controlled source to the same Agent:
printf 'Meeting notes...' | cargo run -p local-bridge -- agent codex -
```

Configure the first optional CloudFile integration **locally** (not from an AI prompt):

```bash
# Point to the installed CloudFile 2.3 native Host or fixed Launcher.
export LOCAL_BRIDGE_CLOUDFILE_HOST=/absolute/path/to/cloudfile-launcher
cargo run -p local-bridge -- status
```

An MCP client can launch `local-bridge mcp` with **stdio**, without any listening HTTP port. Currently offered tools: `bridge.status`, `cloudfile.status`. Neither tool modifies files or checks out a repository.
**New opt-in CLI integration:** `agent codex <prompt|->` calls the user's existing authenticated Codex CLI in ephemeral, read-only mode, and reports `completed` only after `turn.completed` and successful process exit. This does not create a BridgeTask database, use browser SSO, read arbitrary chats, provide a messaging connector, or make agent execution callable by arbitrary webpages/MCP clients. Missing Codex/login or an incomplete run reports an error. Codex CLI read-only and App Server handshake/task have been tested on one macOS development machine; the full Bridge App Server Adapter and real Chrome UI / Windows E2E are still unverified. A missing CloudFile binary reports disconnected rather than trying an unrelated command.

Example Channel configuration (not authenticated or automatically fetched):

```bash
cargo run -p local-bridge -- validate-channel examples/enterprise-channels.json
# 旧 ChannelManifest 示例带短期 expires_at，有效期过后需更新到当前时间起 7 天内
```

Read [security boundaries](docs/SECURITY.md) before integrating Web or AI workloads. Schema validation alone never authenticates an enterprise server.

## Existing repositories

- [CloudFile Agent2 / 2.3](https://github.com/eaopen/cloudfile-local-agent2): first application’s file/checkout/commit engine; independently maintained.
- [CloudFile](https://github.com/eaopen/cloudfile): CloudFile product family.
- [CloudFile Chrome Extension](https://github.com/eaopen/cloudfile-chrome-extension): browser-native file editing; no automatic takeover by this project.
- [Local Tool Bridge Chrome Extension](https://github.com/eaopen/local-bridge-chrome-extension): independent generic Chrome MV3 extension; optional Native Messaging handoff.
- [WeChat Provider](https://github.com/eaopen/wechat-provider): independently installed MCP/SQLite/UI automation provider; optional user-approved CLI integration.

## Product principles

1. **真实痛点先行**：先减少用户步骤，再建设通用框架。
2. **默认混合，按需隔离**：个人与企业可共用已授权能力及任务视图。
3. **复用已有系统**：SSO、CloudFile、Agent、MCP、通讯程序不重复造轮子。
4. **尽可能无感**：已登录不重复登录，已安装不重复安装，来源不同不额外审批。
5. **开放真正可用的 Connector**：第三方可在用户许可范围内提供消息读取/订阅/发送，实际接口能力需核实。
6. **简单可靠的责任边界**：验证调用来源、能力与原系统授权，不开发企业 DLP、庞大任务账本、任意命令执行平台。

未来开发请先读 [AGENTS.md](AGENTS.md) 与 [使用场景](docs/PRODUCT_POSITIONING_AND_USE_CASES.md)。

License: Apache-2.0.

For MCP client setup, see [MCP stdio guide](docs/MCP.md).

## WorkBuddy CLI 增量（2026-10-28）

Bridge → WorkBuddy 官方 Local Assistant 的 `status/send/messages` 已实现，详见 [CLI 使用与限制](docs/WORKBUDDY_CLI.md)。此前未实现的记录为历史基线；当前已提供 CLI 适配，但 OAuth 实机调用仍待验收。客户端 UI 短任务成功不代表 API 授权或此适配器端到端成功。

## 本机 Agent 验收（2026-10-28）

Codex 既有 CLI/App Server、WorkBuddy 原桌面账号 WBIPC + 内置 CLI/ACP、Pi RPC 均有真实任务证据。WorkBuddy/Pi 的 Rust 实验入口已验证写文件及独立结果读取；它们尚不是通用正式 Adapter，也没有 Web/Chrome 接线。Pi 成功使用已有 9router 扩展，默认 MiniMax 直连仍有 401 问题。见 [统一验收记录](docs/AGENT_PROVIDER_VALIDATION.md)、[WorkBuddy 实验说明](scripts/workbuddy-native-validation/README.md)。这些可选实机实验不在 CI 中调用模型。
