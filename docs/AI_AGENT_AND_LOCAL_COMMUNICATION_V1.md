# Generic Local Bridge — Agent Providers, MCP and Local Communications V1

状态：**整体仍是规划合同，但若干独立切片已落地**。Bridge 当前已有只读 MCP Server（`bridge.status`、`cloudfile.status`）、离线 Profile 校验、显式 Codex CLI run、Chrome 手动收件箱，以及可由本机用户主动注册并调用的通用 MCP Client。后者与独立 [WeChat Provider](https://github.com/eaopen/wechat-provider) 的六工具声明、`wechat_status` 和空索引 `wechat_search` 已完成无账号的跨进程 CI 验证。**未完成**可信企业 Web/SSO、自动聊天事件订阅、微信桌面真实收发验收、通用 AgentAdapter/任务中心。本文不要求安装任何特定模型/Agent，不改 CloudFile 文件引擎。

参考：[Codex/WorkBuddy 实测及验收矩阵](AGENT_PROVIDER_VALIDATION.md)、[ApplicationProfile V1](APPLICATION_PROFILE_V1.md)、[BridgeTask V1](BRIDGE_TASK_V1.md)、[Capability/MCP V1](CAPABILITY_AND_MCP_V1.md)、[CloudFile 迁移边界](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

## 1. 五项通用产品能力

| 通用模块 | 职责/边界 |
|---|---|
| `bridge.tasks` | 文件、AI、企业动作、通讯的统一任务展示和跨模块分派，**不代替**领域模块原始状态 |
| `cloudfile.files` | 本地查看、普通编辑、独占编辑及软件关联；文件授权、Journal、Guard 与版本发布始终由 CloudFile 管理 |
| `ai.agent` + `ai.mcp` | 本地多 Agent 适配、MCP 工具提供与消费；用户本机选择 Agent，企业应用不能决定具体供应商 |
| `enterprise.app` | 任意业务应用通过 ApplicationProfile 声明 Web/业务服务与请求能力；Bridge **不识别或导入特定企业应用** |
| `communication.local` | 通用通讯连接器：用户许可的官方或第三方本地 MCP/SDK/IPC/桌面能力可按实际支持提供消息读取、订阅、发送、状态查询；人工分享仅为可选后备 |

本地通讯与 AI Agent 的可用性不得成为文件查看、编辑、退出和更新的必需前提。

**产品默认：个人与企业混合协作。**同一台电脑可同时执行企业 CloudFile 文件任务与个人微信助手任务，共享已授权 Agent/MCP 能力；Profile 不是默认数据墙，也不增加单独的 Bridge 跨 Profile 审批。纯个人本地工作空间（PersonalProfile）为轻量配置概念，不替代 ApplicationProfile V1。

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
- **Bridge 作为 MCP Client/适配器（已实现最小 CLI V1）**：本机用户通过 `provider add/tools/call/remove` 注册已安装的固定 stdio Provider 并显式调用真实 MCP 工具。与 WeChat Provider 的协议集成见 [本机 MCP Provider 接入](LOCAL_MCP_PROVIDER_V1.md) 与 [无账号真实进程烟测](WECHAT_PROVIDER_MCP_SMOKE.md)。**尚不**把注册的读取/发送工具自动透传给本机 Agent、Chrome 网页或未授权 MCP 客户端。未来工具级授权以用户实际需求为准，不复制各厂商工具总线。
- **AI 与文件权限**：企业上下文和权限由业务应用提供，CloudFile 文件 ACL/写入仍由 CloudFile 服务端终判。AI 生成的参数与人工输入使用同一执行授权入口。
- **任务关系**：每一次 Provider Agent run 产生至多一个主 `BridgeTask`；对话消息不是独立任务。`owner_module` 可以是 `ai.workbuddy` / `ai.codex` / `ai.pi`，实例任务 ID 映射但不替换原会话 ID。

## 4. 通讯目标：真正的消息读取、订阅与发送

用户希望用本机 AI 和业务/文件工具处理聊天、群消息与结果，而不只是点击“打开微信”再复制粘贴。Local Bridge 应通过**用户安装并许可的第三方或官方 Connector**接入实际可用的聊天能力。用户自己选择账号、聊天/群会话和允许的读写范围。

### 4.1 接入路线，不绑定特定软件

| 路线 | 使用方式与能力边界 |
|---|---|
| 官方 API / 企业消息 SDK | 有权限时优先复用真正的消息/事件/发送接口 |
| 用户安装的第三方本地 MCP / SDK / IPC Connector | 通过受控 CommunicationAdapter 接入，只暴露它真正具备的能力；无需假设个人通讯软件提供官方公共 API |
| 应用许可的本地桌面交互 | 可作为辅助方式；区分窗口可见内容、完整聊天历史及真正的发送结果 |
| 人工复制/分享或 WorkBuddy 微信助理 | 可以是可选适配路线，不应成为所有通讯集成的唯一要求 |

**微信、Soul 只是用户明确提出的场景，不代表官方私聊 API 已存在，也不代表 Bridge 已实现读取或发送。** 不内建破解私有协议、注入客户端、获取第三方账号密码、规避应用保护的能力。用户授权不自动覆盖平台条款、第三方隐私或企业已有终端策略。

### 4.2 目标 CommunicationAdapter V1（尚未实现）

- `accounts.list` / `conversations.list`：发现已接入的账号与其允许访问的聊天/群会话。
- `messages.read` / `messages.subscribe`：读取指定聊天、订阅其新增消息事件；只有通知摘要的 Connector 不得谎称完整历史可读。
- `messages.send` / `messages.status`：准备回复或按用户预设规则真正发送，并查询同一次操作的结果；`prepared` / `accepted` / `opened` 不等于 `delivered`。
- 历史搜索、附件及高级事件按真实连接器能力和需求逐步扩展，未实现时标不可用。
- 请求与事件关联连接器、账号、会话、消息 ID/时间和任务引用；结果未知先查询原操作，不盲目重复发送。

### 4.3 用户自己决定如何集成

- **默认混合**：个人聊天消息 → 本地 Agent → 用户有权使用的 CloudFile/业务工具 → 草稿或受权发送。Bridge 不因个人和企业 Profile 不同自动阻断正常协作。
- 首次启用 Connector 由用户确认来源、账号、会话范围和允许读写的能力；可以选择“发送草稿需确认”或“指定聊天/规则下允许自动发送”，不必每条普通读取都重复确认。随时可停用。
- Bridge 不是通讯平台的数据仓库。聊天正文/附件属于 Connector 或用户选定存储，BridgeTask 只保留必要关联与真实状态。
- 来自网页、群聊天或模型的文本是输入，并不直接授权任意本机程序、消息外发或文件删除；业务服务仍管业务授权，Bridge 只管自身受信调用。

## 5. 安全和环境

- 企业服务器申请 `ai.agent` 不意味着在本机自动启动 WorkBuddy、Codex 或 Pi。敏感文件发给第三方模型/云服务之前必须按业务策略和用户操作明确授权。
- Agent Provider 的就绪定义：已安装 + 接口可调用 + 已登录/第三方授权 + 对本次任务开放所需能力；单纯发现进程或安装目录不足。
- WorkBuddy 开放平台如需 `client_secret`，应位于可信服务端/供应商授权端，不能编译进通用 Bridge 或应用 JavaScript。
- 运行中人工授权、文件提交、消息发出不得因 Agent 断线自动换 Provider 重新执行；可展示受控重试。
- Dev/Prod Profile、设备身份、用户工作区、MCP 本地连接权限互不串用。
- 禁止远端 Profile 或 Agent 下发任意 EXE、Shell、程序参数、未经筛选的文件路径。

## 6. 最小垂直交付

1. 先用已有本机 Codex/WorkBuddy 或可选 Pi 做一次真实 discover/鉴权/Agent run，Provider 未授权则如实显示不可用。
2. 一个个人或企业入口调用真实只读 MCP，并返回结果；BridgeTask 只聚合任务，不重做 AI 会话。
3. **选择一种实际可用的 Connector**，在用户许可的指定会话内验证“读取/新增事件 → Agent 处理 → 草稿或受权发送 → 查询结果”。没有接口的通讯软件不应假装可用。
4. 同一台 Windows 电脑并发验证 CloudFile 已有文件编辑和个人微信助手任务，默认共享已授权工具，保持 CloudFile Host/SSO/Guard/Journal 原行为。
5. macOS/Linux 渐进，后续根据真实效果扩展通讯和企业连接器；不做大型通讯抓取平台、企业 DLP 或第二套文件引擎。

**本合同不是运行代码交付；涉及具体应用的 Web 路由、业务模型、AI 接口属于应用项目，不加入此通用仓库。**
