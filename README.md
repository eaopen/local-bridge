# Local Bridge

**面向个人与企业的轻量级本地能力连接与任务协作平台。**

*Connect the apps, files, AI agents and communication tools people already use — on their own computer.*

**最高产品原则：让用户更方便协作。** 将本机原本割裂的平台连接起来，解决复制粘贴、重复登录、导入导出、工具切换和任务难以追踪等实际困扰，而不是让用户维护新的复杂系统。

[个人/企业/混合使用场景](docs/PRODUCT_POSITIONING_AND_USE_CASES.md) · [混合 Profile 与 SSO](docs/PROFILE_AUTH_AND_COLLABORATION.md) · [Architecture](docs/ARCHITECTURE.md) · [Roadmap](docs/ROADMAP.md) · [Security](docs/SECURITY.md)

> **当前实现：0.1.0 只读 MCP + 离线 Profile 校验原型。** 可信绑定、个人工作空间、Agent run、通讯双向连接和任务聚合尚未交付。CloudFile 本地文件编辑继续由 [CloudFile Agent2](https://github.com/eaopen/cloudfile-local-agent2) 负责，不由本仓接管。

## 为什么需要 Local Bridge

同一台电脑上，企业 Web、个人微信等通讯、CloudFile、Office/CAD 和已有 Codex/WorkBuddy/Pi 往往各自工作，用户要反复复制消息、下载文件、切窗口和重新登录。

Local Bridge 提供**最小的本机能力连接层**：
- **企业：**已登录的 Web 可经原应用授权调用 CloudFile、本机专业软件、MCP 和 Agent。
- **个人：**复用已安装的 AI/MCP、个人文件和获用户许可的通讯 Connector；微信、Soul 等是否可读/订阅/发送取决于实际第三方接入能力。
- **混合：**同一工作电脑同时编辑企业文件和运行个人聊天助手，**默认混合、按需隔离**，不强制切换 Profile 或增加额外的跨 Profile 审批。
- **渐进兼容：**不安装 Bridge 时现有企业 Web、CloudFile 文件三模式和本地工具仍正常使用。

Profile 用于来源、账号上下文与任务归属，**不默认作为个人/企业数据墙**。已有应用负责 SSO、业务权限与文件版本；Bridge 只承担自身可信调用、能力分发与任务交接，不建设新 DLP 或企业认证中心。

## 通用 Profile 与长期演进合同

**一个 Runtime 同时支持个人和企业。** 有服务端的企业或个人 Web 应用使用 ApplicationProfile V1；纯本机个人工具使用规划中的轻量 PersonalProfile/默认个人工作空间（尚非已交付的新 Schema）。两者共享通用 Capability 和 BridgeTask，默认混合；Bridge 核心仍不内建企业应用业务规则。

- [ApplicationProfile V1](docs/APPLICATION_PROFILE_V1.md)：应用配置、可信来源、Dev/Prod、更新及稳定身份
- [BridgeTask V1](docs/BRIDGE_TASK_V1.md)：统一文件/企业/AI 任务中心合同
- [Capability / MCP V1](docs/CAPABILITY_AND_MCP_V1.md)：通用工具与 MCP 权限
- [Agent Providers / Local Communications V1](docs/AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)：WorkBuddy / Codex / Pi 本地选择、微信优先通讯
- [CloudFile → Bridge 迁移](docs/CLOUDFILE_TO_BRIDGE_MIGRATION.md)：并行开发不接管或破坏现有文件能力
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
| `crates/local-bridge` | CLI and local MCP stdio endpoint | Read-only prototype |
| CloudFile adapter | Connects installed Agent2 via a user-configured executable | Read-only `--component` probe |
| Enterprise Channels | Server describes allowed navigation and built-in capabilities | Offline manifest validation only |
| File editing / Checkout | CloudFile’s actual file workflows | Continue in the existing Agent2 repo; **not yet bridged into MCP** |

## 构建及本地测试

Requires Rust **1.90+**. No Node.js, Python, Docker or model runtime required at runtime.

```bash
cargo test --workspace
cargo run -p local-bridge -- status
cargo run -p local-bridge -- mcp
```

Configure the first optional CloudFile integration **locally** (not from an AI prompt):

```bash
# Point to the installed CloudFile 2.3 native Host or fixed Launcher.
export LOCAL_BRIDGE_CLOUDFILE_HOST=/absolute/path/to/cloudfile-launcher
cargo run -p local-bridge -- status
```

An MCP client can launch `local-bridge mcp` with **stdio**, without any listening HTTP port. Currently offered tools: `bridge.status`, `cloudfile.status`. Neither tool modifies files or checks out a repository. A missing CloudFile binary reports disconnected rather than trying an unrelated command.

Example Channel configuration (not authenticated or automatically fetched):

```bash
cargo run -p local-bridge -- validate-channel examples/enterprise-channels.json
# 旧 ChannelManifest 示例带短期 expires_at，有效期过后需更新到当前时间起 7 天内
```

Read [security boundaries](docs/SECURITY.md) before integrating Web or AI workloads. Schema validation alone never authenticates an enterprise server.

## Existing repositories

- [CloudFile Agent2 / 2.3](https://github.com/eaopen/cloudfile-local-agent2): first application’s file/checkout/commit engine; independently maintained.
- [CloudFile](https://github.com/eaopen/cloudfile): CloudFile product family.
- [CloudFile Chrome Extension](https://github.com/eaopen/cloudfile-chrome-extension): browser-native connection; no automatic takeover by this project.

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
