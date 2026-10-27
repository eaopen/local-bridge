# Capability / MCP / Pi V1 — 通用能力合同

状态：**设计基线**。当前 MCP stdio 已实装且仅开放 `bridge.status`、`cloudfile.status`；其他下述能力 ID/接口属于待实现目标，不可标记为已可用。

## 1. 依赖方向

```text
Enterprise App / Browser / AI Client
      |  authenticated app profile + per-action grant
Generic Browser Extension / MCP Client
      |
Local Bridge (trusted caller + capability registry + task UI)
      |                     |
CloudFile file module       Pi RPC runtime (optional)
      |                     |
existing Hub/Server         Bridge MCP stdio tools
```

- Bridge **不导入**企业应用代码，不识别固定业务系统、项目表、员工标识或企业私有 API；ApplicationProfile 来自企业应用。
- CloudFile 和 Bridge 可以相互知道，并通过稳定模块 ID、最小 IPC 与任务摘要适配；CloudFile 引擎仍拥有文件权限和版本操作。
- Pi 作为可选独立进程。运行时可使用 Pi RPC 接入界面；Pi 作为 MCP Client 访问 Bridge 的受限工具；其他 MCP Client 同理。不要在 Rust 内实现第二个 AI Agent 引擎。
- MCP Server 默认 `stdio`，不开放任意网站可调用的本机 TCP 端口。

## 2. 能力注册、广告与真实授权

能力 ID 使用命名空间（例如 `cloudfile.status`、`cloudfile.file.view`、`cloudfile.file.edit`、`cloudfile.file.exclusive`、`bridge.task.list`、`enterprise.action.invoke`、`ai.local.chat`）。

严格区分：
1. **Declared**：ApplicationProfile 请求使用的能力；
2. **Installed**：本地 Bridge 已有并可调用的能力模块；
3. **Available**：模块/服务健康，当前用户/来源允许操作；
4. **Authorized**：对本次对象/任务持有可信业务授权，包括 CloudFile 的文件票据和 ACL。

只有 4 同时成立才能执行。Web 入口可显示不代表具有本机命令权限。未知模块/能力为不可用而非默认透传。

## 3. 本地能力的类型安全合同

本地执行只接受已编译/已安装的注册能力、固定工具名称及受限参数结构。不得从远端 Profile、网页或模型提示中读取任意 shell、EXE 路径或 argv 并执行。

示例目标工具（**尚未实现**）：

| 工具 | 归属模块 | 最低授权 |
|---|---|---|
| `bridge.task.list` | Bridge | 当前可信 Profile 下的可见任务 |
| `cloudfile.task.list` | CloudFile | 当前用户文件任务摘要，去敏 |
| `cloudfile.file.open` | CloudFile | 既有短期只读文件授权 |
| `cloudfile.file.edit` | CloudFile | 受控写票据；本机启动不代表发布成功 |
| `cloudfile.file.exclusive` | CloudFile | 有效 ACL + Guard，用户明确 Checkout |
| `cloudfile.file.commit` | CloudFile | 既有 Journal/原生条件提交和人工确认 |
| `enterprise.action.invoke` | 企业适配模块 | 已注册动作 ID + 企业对象权限 + 必要确认 |

首次上线优先只读任务、能力发现，再逐步开放写操作。不要为了 MCP 重新写 CloudFile 发布或独占逻辑。

## 4. AI 两种接入方向

**A. Bridge 调用本地 Pi**

Bridge Desktop → Pi RPC (JSONL 子进程) → Pi 的既有会话、事件与模型配置；仅用户/企业启用后启动。BridgeTask 映射 run 级状态，AI 会话仍由 Pi 管理，不能让 Pi 改写 BridgeTask 的文件执行事实。

**B. Pi 或其他 AI 客户端调用 Bridge**

Pi MCP Client → `local-bridge mcp` (stdio) → 注册的 Bridge/CloudFile 工具 → 原模块。Pi 官方已支持 stdio/HTTP MCP Client；无需自己再实现一套 Pi MCP 解析器。对工具的授权需要在 Bridge/目标服务实际执行时完成，MCP 的 tool annotations 不是最终鉴权。

**企业 AI 通道**：企业 AI 应用可通过 Profile 暴露其已有 Web/API；若将来需要访问本机能力，需经过用户绑定的可信网页转发或另行认证的 MCP 通道，不能把来自远程 AI 服务的指令默认为本机用户授权。

## 5. Dev/Prod 与默认限制

- Dev/Prod 使用不同的 Profile/Native Host/本机凭据/工作区，禁止跨环境查询、写回或回退。
- 不因为安装了 Pi 就自动开放本机 shell、整个文件系统、模型凭据或企业密钥。
- 文件查看与写入始终由 CloudFile 决定；企业动作由企业业务服务判断；Bridge 负责本机能力范围与用户确认。
- 变更能力定义只增加小范围合同测试，真实文件发布仍由现有 CloudFile E2E 验收负责。

当前受支持的 MCP 工具及边界以 [MCP.md](MCP.md) 为准，不能将本设计表格误认为已实现功能。

## 6. 通用 Agent Provider 与本地通讯

[Agent Providers / MCP / Local Communications V1](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md) 扩展本合同：WorkBuddy、Codex、Pi 按本机授权和能力选择，Pi 不再是硬编码的唯一 Agent；通讯首选受用户确认的本机微信分享，WorkBuddy 微信助理作为可选受信远程能力。此处新增能力仍属设计，未修改现有 MCP 只读工具。
