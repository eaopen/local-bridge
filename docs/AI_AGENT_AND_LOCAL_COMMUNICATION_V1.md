# Generic Local Bridge — Agent Providers, MCP and Local Communications V1

状态：**统一架构/合同，尚非已交付功能**。Bridge 0.1 目前只有只读 MCP `bridge.status`、`cloudfile.status` 和离线 Profile 结构校验。本文只定义跨厂商接入，不默认要求安装任何模型或 Agent，不改现有 CloudFile 文件引擎。

参考：[ApplicationProfile V1](APPLICATION_PROFILE_V1.md)、[BridgeTask V1](BRIDGE_TASK_V1.md)、[Capability/MCP V1](CAPABILITY_AND_MCP_V1.md)、[CloudFile 迁移边界](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

## 1. 五项通用产品能力

| 通用模块 | 职责/边界 |
|---|---|
| `bridge.tasks` | 文件、AI、企业动作、通讯的统一任务展示和跨模块分派，**不代替**领域模块原始状态 |
| `cloudfile.files` | 本地查看、普通编辑、独占编辑及软件关联；文件授权、Journal、Guard 与版本发布始终由 CloudFile 管理 |
| `ai.agent` + `ai.mcp` | 本地多 Agent 适配、MCP 工具提供与消费；用户本机选择 Agent，企业应用不能决定具体供应商 |
| `enterprise.app` | 任意业务应用通过 ApplicationProfile 声明 Web/业务服务与请求能力；Bridge **不识别或导入特定企业应用** |
| `communication.local` | 本地通讯和消息交接适配；首选受用户确认的桌面微信交接，具备官方授权的其他通道另行接入 |

本地通讯与 AI Agent 的可用性不得成为文件查看、编辑、退出和更新的必需前提。

## 2. AgentAdapter：厂商可替换，选择保存在本机

`ApplicationProfile` 只能声明需要通用能力，例如 `ai.agent.run`、`ai.mcp.tools`；不得从企业服务器强制指定已安装的桌面 Agent，也不得在 Profile 内给出其二进制路径或授权凭据。

本机 `LocalBinding` / 用户设置保存以下信息（**规划字段，并非已实现 Schema**）：

```json
{
  "agent_selection": {
    "mode": "auto",
    "preferred": null,
    "fallback": "pi",
    "enabled_providers": ["workbuddy", "codex", "pi"]
  }
}
```

`mode` 可以是 `auto`、`manual` 或 `disabled`。自动模式仅在**任务开始前**依据用户偏好、任务类型、已安装/可用能力和既有登录选择：办公优先 WorkBuddy，编程优先 Codex，Pi 为可用备选；多种就绪时支持用户手动指定。运行途中、消息发送/文件写入结果未知时不得静默换引擎重试。

统一适配接口可包括 `discover()`、`check_auth()`、`start_run()`、`send_message()`、`get_events_or_poll()`；`cancel()`、`resume()`、`mcp_client()` 为可选能力。未支持不能声称具备。Provider 运行的会话、任务和审批保持在原程序，BridgeTask 只显示外壳/关联 ID。

| Provider | Bridge → Provider | Provider → Bridge（MCP） | 前提/限制 |
|---|---|---|---|
| WorkBuddy | 经开放平台认可的第三方应用和 OAuth 2.1，使用 `/openapi/v2/localassistant` 在线检查、`/message` 发送与历史增量查询；最初可以只打开现有 WorkBuddy UI | WorkBuddy Connector 连接本机 Bridge MCP stdio，或经受信的 MCP 服务接口 | 平台应用注册/授权审核与 Scope 是先决条件；用户有客户端不代表 API 已获授权；机密 Client Secret 不放进桌面/网页 |
| Codex | 本机已安装/登录的 `codex app-server`，优先 stdio，使用官方 JSON-RPC、流式事件与审批 | Codex CLI/IDE 可配置 MCP Server | 不把“Codex 桌面 App 已安装”混同于可启动 CLI/app-server；需探测实际可用版本 |
| Pi | 本机 `pi --mode rpc` / JSONL 事件 | Pi 已支持本地/远程 MCP Client | 可选依赖，保留 Pi 自己的认证、模型与会话，不为 Bridge 用户强制装 Node/Bun |

参考官方资料：
- WorkBuddy Open API：https://open.workbuddy.cn/docs/openapi （本地助理在线、发消息、查历史）
- WorkBuddy Connector：https://open.workbuddy.cn/docs/connector （MCP + Skill 或 CLI + Skill）
- Codex App Server：https://developers.openai.com/codex/app-server/ （如文档路径变更，可参照 https://learn.chatgpt.com/docs/app-server）
- Pi RPC：https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/rpc.md
- Pi MCP：https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md

注意 WorkBuddy **Enterprise 云端 Agent/OpenAPI** 与个人桌面 WorkBuddy 的本地助理 API 是不同产品能力，不可擅自共享业务账户或凭据。

## 3. MCP 双向接入

- **Bridge 作为 MCP Server**：向被批准的 WorkBuddy/Codex/Pi 提供 `bridge.status`、`cloudfile.status`（现有），后续添加 `bridge.task.list`、受控文件能力、通用企业动作。工具不能无条件获得浏览器 Cookie、工作副本路径、企业密钥。优先 `stdio`，不监听公开 TCP。
- **Bridge 作为 MCP Client/适配器**：仅在有明确业务需求、可信服务及用户授权时调用企业/外部 MCP；不复制各种厂商的工具总线，也不将多个远端服务器所有工具无筛选暴露给任意本地模型。
- **AI 与文件权限**：企业上下文和权限由业务应用提供，CloudFile 文件 ACL/写入仍由 CloudFile 服务端终判。AI 生成的参数与人工输入使用同一执行授权入口。
- **任务关系**：每一次 Provider Agent run 产生至多一个主 `BridgeTask`；对话消息不是独立任务。`owner_module` 可以是 `ai.workbuddy` / `ai.codex` / `ai.pi`，实例任务 ID 映射但不替换原会话 ID。

## 4. 通用通信：本地微信优先，但不假设个人微信聊天 API

建议按三条互不混淆的路线：

**A. 本机微信人工分享（首发）**：Bridge 生成一份文本/文件准备态、可打开微信或提供复制/分享入口。用户**自行确认收件对象和实际发送**；无稳定受信回执时状态仅为 `prepared` / `handed_off`，绝不能冒充 `delivered`。不读取个人聊天数据库、不 Hook DLL、不保存用户聊天凭据。

**B. WorkBuddy 微信助理（优先验证的受信远程 Agent 通道）**：官方 Windows/macOS WorkBuddy 可扫码绑定手机微信助理，并通过微信消息驱动本机 WorkBuddy 任务。把 Bridge MCP 工具接入 WorkBuddy 后，微信助理可以在受授权范围内触发 Bridge 工具；**这是 WorkBuddy 的微信通道，不是 Bridge 拥有个人微信聊天收发 API**。没有 WorkBuddy 时不得假装可用。
官方指南：https://www.workbuddy.cn/docs/workbuddy/WeixinBot-Guide

**C. 企业级消息 API（确有需求后）**：企业微信智能机器人/企业微信客服等使用对应官方 API，处理真实双向入站、回执和企业消息身份。此路径与个人微信账号、个人聊天列表完全不同。
参考：https://www.workbuddy.cn/docs/workbuddy/Wecom-Guide

通用接口优先 `communication.prepare`、`communication.open`、`communication.request_send`、`communication.status`；其中自动发送或收件箱读取只在 Provider 明确具备合规、受权官方接口后启用。未能确认实际发送时任务状态为 `needs_action` 或 `unknown`，无重复自动发送。

## 5. 安全和环境

- 企业服务器申请 `ai.agent` 不意味着在本机自动启动 WorkBuddy、Codex 或 Pi。敏感文件发给第三方模型/云服务之前必须按业务策略和用户操作明确授权。
- Agent Provider 的就绪定义：已安装 + 接口可调用 + 已登录/第三方授权 + 对本次任务开放所需能力；单纯发现进程或安装目录不足。
- WorkBuddy 开放平台如需 `client_secret`，应位于可信服务端/供应商授权端，不能编译进通用 Bridge 或应用 JavaScript。
- 运行中人工授权、文件提交、消息发出不得因 Agent 断线自动换 Provider 重新执行；可展示受控重试。
- Dev/Prod Profile、设备身份、用户工作区、MCP 本地连接权限互不串用。
- 禁止远端 Profile 或 Agent 下发任意 EXE、Shell、程序参数、未经筛选的文件路径。

## 6. 最小垂直交付

1. 补齐通用 AgentAdapter 的 **read-only discover/health**，验证 Codex App Server、Pi RPC 与 WorkBuddy 官方接入前提；不因未授权而假装可操作。
2. Bridge MCP 接入 WorkBuddy/Codex/Pi，先展示任务/能力，确保 CloudFile 旧功能独立可用。
3. 让一个有自己 Web UI 的企业应用通过同一个 Profile 同时看到文件、企业 AI 与本地 Agent，工作台不重新实现企业 Chat/SSO。
4. Windows 上先交付用户确认的本机微信分享，随后验证 WorkBuddy 微信助理 + Bridge MCP 的任务闭环；实际回执与用户审批由拥有它的 Provider 报告。
5. 保持 Linux/macOS 核心协议统一，优先 Windows 的应用可用性、文件与通讯体验。

**本合同不是运行代码交付；涉及具体应用的 Web 路由、业务模型、AI 接口属于应用项目，不加入此通用仓库。**
