# Roadmap — 优先交付用户可用的跨系统协作

**双产品栈长期独立（2026-10-29）：** CloudFile Agent2 + 原 CloudFile Chrome 是第一套；Local Bridge + 新 Chrome + 可选独立 CloudFile Provider 是第二套。两套的 Chrome/Host/通用配置和发行保持独立，但**CloudFile 文件目录、Journal/Guard 和文件任务只有 Agent2 一份**，只有明确批准并验收的升级合并才能切换。见[双轨边界](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

**产品定位（2026-10-27）：** 面向个人和企业的轻量级本地能力连接与任务协作平台。**默认混合、按需隔离**。每个开发任务都要证明减少用户操作，而不是增加一个平台。

## M0 — 当前已实现的基线

Rust 核心 0.1.0 + 后续小切片：只读 MCP Server、用户显式注册的 MCP Client、离线 Profile 结构校验、可选 Chrome Native Host/Inbox、Web AI 手动接力，以及用户主动调用既有 Codex CLI 的临时只读任务均已在代码中实现。**Codex/Pi 与 WorkBuddy 均已有独立本机实验记录；统一 Web V2 Agent 状态和 run/read 已有源码，但三者的 Chrome/HTTP 真实端到端尚未验收。已实现代码不等于已验证模型/登录/企业联动。**可信在线 Profile、PersonalProfile Runtime、跨 Provider 任务聚合、完整通信 Adapter、CloudFile 文件 IPC 均未交付。CloudFile 编辑仍走现有 Agent2/Hub/Server。详见 [Agent 实测矩阵](AGENT_PROVIDER_VALIDATION.md)。

## 模型来源与应急方式（2026-10-28 收敛决策）

- **企业：**企业服务方或现有 Web 应用后端提供模型 API、授权、额度与费用；当应用要求使用本机 Pi 时，可由**受信企业会话**提供一次性兼容 API URL/模型/短期授权，让 Pi 无个人 Key 直接消费企业模型。Bridge 只生成一次性隔离 Pi 配置和执行任务，不另建模型网关或用户设置。进程级功能已开发，**浏览器 SSO → 受信票据/自动调用**待端到端接线，不得把普通网页变量直接当凭据。
- **个人：**优先使用用户自己已登录的 Codex / WorkBuddy；Pi 仍是**非必装**备选，只有用户自己已配置可用模型/API 时才使用现有一次性 CLI，**不继续增加 Bridge 自己的 API 配置 UI、计费/额度调度和多提供商路由**。
- **没有 API/Agent：**保留已实现的 Chrome Web AI Relay 人工接力到已登录 ChatGPT/Gemini 等网页，用户自己粘贴、对话和选择回复，Bridge 可接收关联结果；不把消费级网页会话当程序化 API。
- **实际研发重心：**Chrome ↔ Native Host 实机验收、可信 Web 应用调用本机 MCP/Agent 和结果回传，而非再扩张模型配置/免费额度机制。Pi 配置指引仅供主动需要的用户使用，不能作为产品安装门槛或开发阻断。

## M1 — 最短 Web / 本机调用闭环（当前收口）

- 可信 ApplicationProfile + LocalBinding + 能力发现，明确 declared/installed/available/authorized；个人工作空间只做轻量本地配置，不创建云账号或服务端 PersonalProfile Schema。
- 在已登录的企业 Web 经原应用后端短期操作授权、Chrome/Native Messaging 完成一个只读 MCP 工具的真实调用和返回。不新建 SSO；Bridge 不在线时原 Web 正常。
- CloudFile Dev/Prod Host、Journal、凭据与更新来源不互相覆盖。

## M2 — 个人与企业同机工作（下一批真实验收）

- 先提供显式用户触发的 `agent codex <prompt|->` 只读 CLI 接线（完成条件与超时不可误报，需真实本机验收），再扩展到 AgentAdapter/浏览器受控调用；
- **Codex 与 WorkBuddy 两者都列入 P0 验证（不是运行时必装）：** Codex 继续复用实测 CLI，再以 App Server stdio 做会话/状态最小 Adapter；WorkBuddy 先在真实安装机验证本地 Connector → Bridge MCP，获得官方第三方应用 OAuth/Scope 后才验证 Bridge → WorkBuddy Local Assistant Open API。两方向不得混算；Pi 是可选备选。见 [验收标准](AGENT_PROVIDER_VALIDATION.md)。
- Agent2 `cloudfile-native-host --bridge-provider` 的只读摘要逐步接入 BridgeTask，展示**同一个 Agent2 文件数据根**的现有任务；不在 Bridge 新建 Journal/工作副本或并行上传引擎。
- 同一台公司电脑同时运行企业文件任务和个人聊天助手任务；不切换 Profile、不增加跨 Profile 审批，统一展示真实结果。

## 已实现的本机 Provider 连接基线（仍待真实微信实机验收）

- 用户显式注册已安装 MCP stdio Provider，Bridge 作为 Client 通过 CLI 完成 `initialize/tools/list/tools/call`，第一个接入示例为独立 WeChat Provider。此注册**不会**把写入/发送工具自动开放给网页和 AI。
- 原 Provider 继续管理账号、SQLite、发送结果与幂等；Bridge 不重建聊天系统，也不声称其 UI 兼容已通过真实微信验证。先在用户主动操作的受控聊天完成真实读写验收。

## M3 — 通讯双向连接 + 企业 Web AI 增强

- 使用一种**确实可用且获用户许可**的第三方/官方通讯 Connector，完成指定聊天消息读取/新消息订阅 → Agent → 草稿或授权发送 → 发送状态查询。微信、Soul 是场景，不预设其官方个人聊天 API。
- Etech 元枢复用原 Web、SSO 与 AI API，仅添加通用 MCP/Agent 本地能力。不把企业代码或固定地址放进 Bridge 核心。
- 可选的个人/企业隔离设置仅在真实需求下追加，不作为默认连接的 P0 阻断。

## M4 — 验证后才考虑统一桌面入口

- 为**第二套 Chrome/Bridge 入口**逐项验证查看、普通编辑、独占编辑，经 Provider 调用原 Agent2 唯一文件执行器；两入口共用文件任务、Journal/Guard/工作副本，不独立复制。
- Windows 普通用户优先，Mac/Linux 同合同逐步覆盖；Chrome/Agent 升级与回退须保持已有生产使用习惯。
- 第二套可以按实际价值添加 Bridge 可选的通用桌面任务壳；第一套原 Tauri 长期保留。统一安装、旧插件退役属于未来单独批准的升级合并，不是本里程碑自动结果。

**判断标准：**解决了什么痛点、减少哪些步骤、复用了什么、有没有新障碍。拒绝重复 SSO、企业 DLP、重写文件引擎、大型通用 RPA、重复任务数据库和过度审批。

参考：[个人/企业场景](PRODUCT_POSITIONING_AND_USE_CASES.md)、[混合 Profile/SSO](PROFILE_AUTH_AND_COLLABORATION.md)、[通讯/Agent](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)、[CloudFile 迁移](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。
