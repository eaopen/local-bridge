# Local Bridge

**面向企业内网的轻量、安全、可扩展的 Rust 本机能力桥接器。**

*Lightweight, secure and extensible Rust bridge between enterprise Web applications, AI agents and local desktop tools.*

[中文说明](#为什么需要-local-bridge) · [Architecture](docs/ARCHITECTURE.md) · [Security](docs/SECURITY.md) · [Roadmap](docs/ROADMAP.md)

> Status: **0.1.0 architecture + runnable read-only MCP scaffold**. NOT a production-ready file editor. CloudFile editing continues in the existing [CloudFile Agent2](https://github.com/eaopen/cloudfile-local-agent2). No automatic AI operations, remote script execution or enterprise Channel fetching are enabled by this repository yet.

## 为什么需要 Local Bridge

企业 Web 应用和 AI Agent 可以理解任务、调用云端 API，却不总能直接使用用户电脑里的专业软件、企业专有 EXE、文件和设备。

Local Bridge 提供一个 **最小、受控的本机执行接口**：
- 企业业务系统通过现有 API 与本机桥接入口协作。
- WorkBuddy、Claude、Codex 或其他 MCP 客户端使用受批准的工具，而不是依赖某一大模型厂商。
- 第一个业务集成是 **CloudFile**：本地打开/编辑、Checkout/Checkin、版本上传仍由已存在的 Agent2.3 核心执行，不复制 Seafile 或 Journal 实现。
- 后续企业功能以 **Channel manifest + 本机能力白名单**扩展，不允许远程服务器下发 Shell/EXE 命令。
- 优先支持 Windows 普通用户与便携运行、macOS；Rust 核心预留 Linux。支持企业服务端 HTTP 和 HTTPS，敏感授权另行处理。

## 通用 Profile 与长期演进合同

**本仓只认识通用 ApplicationProfile，不内建任何企业应用的业务规则。** 企业应用自行发布和维护其 Profile；CloudFile 与 Bridge 则可以双向适配文件能力。

- [ApplicationProfile V1](docs/APPLICATION_PROFILE_V1.md)：应用配置、可信来源、Dev/Prod、更新及稳定身份
- [BridgeTask V1](docs/BRIDGE_TASK_V1.md)：统一文件/企业/AI 任务中心合同
- [Capability / MCP / Pi V1](docs/CAPABILITY_AND_MCP_V1.md)：通用工具、Pi RPC、MCP 权限
- [CloudFile → Bridge 迁移](docs/CLOUDFILE_TO_BRIDGE_MIGRATION.md)：并行开发不接管或破坏现有文件能力

```sh
cargo run -p local-bridge -- validate-profile examples/application-profile.json
```

`validate-profile` **仅验证 JSON 结构**，返回 `trusted=false` / `activated=false`；不会下载配置或注册 Host。现有 `validate-channel`、MCP 的 `bridge.status` 和 `cloudfile.status` 完全保持原样。

## 结构

```text
AI Agent (MCP stdio)        Enterprise Web (browser/API)
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

## Principles

1. **Use existing AI agents** instead of developing another chat assistant.
2. **Keep the default runtime small**: no WebView, filesystem drivers, privileged service, full disk sync, or remote code loading.
3. **Local capabilities are explicit and auditable**: the enterprise config may advertise an action but cannot grant it.
4. **Enterprise-owned workflows, vendor-neutral AI**: business permissions and authoritative file versions remain server-side.
5. **Real first, platform second**: only expose tools whose implementation and authorization can actually be tested.

License: Apache-2.0.

For MCP client setup, see [MCP stdio guide](docs/MCP.md).
