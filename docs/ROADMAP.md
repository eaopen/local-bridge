# Roadmap — 优先交付用户可用的跨系统协作

**双产品栈长期独立（2026-10-29）：** CloudFile Agent2 + 原 CloudFile Chrome 是第一套；Local Bridge + 新 Chrome + 可选独立 CloudFile Provider 是第二套。两套的 Chrome/Host/通用配置和发行保持独立，但**CloudFile 文件目录、Journal/Guard 和文件任务只有 Agent2 一份**，只有明确批准并验收的升级合并才能切换。见[双轨边界](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

**产品定位（2026-10-27）：** 面向个人和企业的轻量级本地能力连接与任务协作平台。**默认混合、按需隔离**。每个开发任务都要证明减少用户操作，而不是增加一个平台。

## 2026-10-29 实际交付与验收状态

### macOS 实机新增验收（2026-10-29）

已在独立 Chrome for Testing 156 配置下验证 **真实 Chrome → 独立 Native Host → Bridge → 本机 MCP 只读 Provider → 浏览器结果**，包括 Origin 授权后调用与撤销后拒绝；通过已有 Codex 桌面登录的 CLI，完成 **Web V2 `runAgent/readAgent` → 明确 `task.completed`**，并验证同一 `client_key` 再调用返回 `reused:true`、不启动第二个任务。可选 Tauri DEV .app 实际启动并显示窗口；AgentPreferences 后端保存/回读通过。验收记录：[macOS Chrome/Agent/Tauri 实机报告](validation/chrome-macos-20261029.md)。

范围限制：测试扩展预先授予仅测试用户配置的权限，**未测试正式权限弹窗 UX**；Chrome HTTP/SSE → Native 断线续读、Tauri 窗口内保存/重启验收、Windows 浏览器真机、企业 SSO 和 CloudFile 文件端到端仍未完成。开发预览构建 [PR #38](https://github.com/eaopen/local-bridge/pull/38) 已合并，Windows/macOS unsigned artifacts CI 通过，尚无正式签名 Release。

最近五批已合入 Local Bridge `dev`：[Native Host 生命周期 #32](https://github.com/eaopen/local-bridge/pull/32)、[Web 授权原子存储 #33](https://github.com/eaopen/local-bridge/pull/33)、[只读任务聚合 #34](https://github.com/eaopen/local-bridge/pull/34)、[AgentPreferences #35](https://github.com/eaopen/local-bridge/pull/35)、[可选 Tauri Desktop #36](https://github.com/eaopen/local-bridge/pull/36)。`dev` 基线 `2955732` 的 Windows/macOS/Ubuntu 核心 CI **通过**；独立 Tauri Windows/macOS 编译和静态 UI CI **通过**，但**不等于真实 GUI 安装/运行验收**。尚无正式 Release、签名安装包。

| 方向 | 已实现的代码/验证 | 还未交付，不能宣称已上线 |
| --- | --- | --- |
| Core / CLI / MCP / Native | MCP stdio、用户注册 Provider、Chrome Host、Inbox、诊断/回退/授权持久化 | 普通用户机器上的真实 Chrome 安装/绑定与正式发布 |
| Chrome Web V2 / Agent | 独立 Chrome MV3 + Web SDK、AgentPreferences；**macOS 实机已通过只读 MCP 及真实 Codex `runAgent/readAgent`、同 clientKey 去重、Origin 撤权** | Windows、正式权限弹窗、HTTP/SSE → Native 断连续读、企业 Web 登录；其他 Agent 的 ready 不等于已验证 |
| Agent | Codex/Pi 文本 CLI Worker、WorkBuddy 独立真实实验回执、重复 clientKey 保护 | 通用 WorkBuddy 原桌面账号 Adapter、实际模型/ThinkMode 生效核验、多平台失败恢复 |
| 任务中心 | 本机 CLI 和独立 Tauri 预览的只读 AI/WorkBuddy/Inbox 任务聚合 | 通用 Web 任务 API、事件订阅、跨模块动作转发；来源错误要保留不完整状态 |
| CloudFile 文件 | Agent2 独立文件 Owner 保留；[Provider PR #26（已合入 Agent2 dev）](https://github.com/eaopen/cloudfile-local-agent2/pull/26) 有共用原 Agent2 数据根的只读脱敏代码 | Provider 已合并 Agent2 `dev`，未自动启用或注册；无 Bridge 三模式真实文件动作，不能建第二 Writer |
| 企业 V2 / Profile | ApplicationProfile V2 **离线结构解析/校验**、企业 Pi 临时推理适配代码 | 可信在线 Profile/SSO/企业 AI API 的真实 Web 联动 |
| 通讯 Provider | Bridge MCP Client 注册/调用独立 Provider，微信状态/搜索有协议/CI 测试 | 真实微信 GUI、用户授权和消息读/发的实机验收 |

**下阶段 P0：**macOS 真实 Chrome + Native Host + 只读 MCP 及真实 Codex Agent 任务已通过，改优先验证 **Windows 真机 / 正式权限点击 / HTTP-SSE 断线转 Native 续读**，继续在 [#9](https://github.com/eaopen/local-bridge/issues/9)、[#6](https://github.com/eaopen/local-bridge/issues/6) 跟踪；补齐 Tauri 窗口内设置保存与重开回读。签名发布、企业登录和 CloudFile 写入均未包含本轮。**不再新增任务框架、账号网关或文件引擎。**

其后才按实际依赖推进 CloudFile 同一文件根的只读 Provider 验证、可信企业 Profile/模型 API、WorkBuddy 正式 Adapter、独立微信 Provider。原 Agent2 Tauri 与两套 Chrome/Host 独立并行，旧客户端不自动退役。

## M0.1 — 首批从 CloudFile Agent2 提取通用能力（2026-10-29）

- **Native Host 生命周期**：仅在 Local Bridge 自己的 `com.eaopen.browser_bridge` 下完善用户级注册状态、备份后显式替换、卸载、回退；旧 CloudFile Host 完全不动。
- **轻量自检**：借鉴 Agent2 的“已配置/未配置/真正失效”区分；本机只检查私有目录可读写、Bridge Host Manifest 和用户 MCP 注册，不把 Chrome GUI、Agent 登录或 CloudFile 写入标成已验证。
- **保持体积与依赖**：不新增常驻桌面进程、第二套 CloudFile 工作副本、Journal/Guard、Tauri/WebView2，也不因第一批增加数据库、自动更新服务或新业务参数。
- 这是首批的历史规划：其后本地授权原子写入与只读任务投影已在 M0.2/M0.3 实现；自启、签名更新和发布回退仍应在真实用户安装需求明确后处理。详见[小步迁移计划](LOCAL_FOUNDATION_MIGRATION.md)。

## M0.2 — Bridge Web 授权原子存储（第二批）

从 Agent2 的原子写入/短事务原则提取极小的通用状态函数，解决固定 `.json.tmp` 残留和跨进程授权互相覆盖。仅修复 Bridge 自身 status/Agent/WorkBuddy 三组 Web 授权；存储路径/JSON schema 不变，不需要新依赖，也不涉及 CloudFile Journal、Guard 或工作副本。详细范围见[通用能力迁移计划](LOCAL_FOUNDATION_MIGRATION.md)。

## M0.3 — 只读任务聚合 + AgentPreferences（已合并）

[PR #34](https://github.com/eaopen/local-bridge/pull/34) 交付 `tasks list [--cloudfile]`，聚合已有 AI/WorkBuddy/Inbox 任务事实源；CloudFile 仅在用户显式要求且 Agent2 Provider 已注册且兼容时读取脱敏摘要。[PR #35](https://github.com/eaopen/local-bridge/pull/35) 交付本机 AgentPreferences、优先顺序与 Model/ThinkMode，统一 `runAgent` 仅对未填写的参数使用默认值；显式参数、Origin 用户授权和任务 Owner 终态仍优先。**这不是通用 Web Task API，也没有创建新的任务数据库。**

## M0.4 — 可选本机 Tauri Desktop 管理入口（独立预览）

基于已交付的 `tasks list`、`self-test`、Native Host 状态和 AgentPreferences，本轮建立独立 Tauri 2 单窗口管理应用（概览/只读任务/Agent 默认配置）。Desktop 只调用本机 Rust Commands，显式选择后才读取 CloudFile Provider；默认不运行 Agent、不修改文件、不安装常驻服务。它是**与 Bridge 核心解耦的开发预览**，完整 GUI/发行验收和签名自动升级未完成，也不替代 CloudFile Agent2 现有 Tauri。参见 [Desktop 说明](../crates/local-bridge-desktop/README.md)。

## M0 — 初期实现基线（历史，当前状态见上方核查表）

Rust 核心 0.1.0 + 后续小切片：只读 MCP Server、用户显式注册的 MCP Client、离线 Profile 结构校验、可选 Chrome Native Host/Inbox、Web AI 手动接力，以及用户主动调用既有 Codex CLI 的临时只读任务均已在代码中实现。**Codex/Pi 与 WorkBuddy 均已有独立本机实验记录；统一 Web V2 Agent 状态和 run/read 已有源码，但三者的 Chrome/HTTP 真实端到端尚未验收。已实现代码不等于已验证模型/登录/企业联动。**可信在线 Profile、完整 PersonalProfile Runtime、**通用 Web 任务 API / 事件订阅 / 动作转发**、完整通信 Adapter、CloudFile 文件操作 IPC 均未交付；本机 CLI/Tauri 只读聚合已在 M0.3/M0.4 完成。CloudFile 编辑仍走现有 Agent2/Hub/Server。详见 [Agent 实测矩阵](AGENT_PROVIDER_VALIDATION.md)。

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
- Bridge 可选管理 Tauri 的**开发预览已提前落地**（M0.4），但仍需真实安装和 GUI 验收；原 Agent2 Tauri 长期保留。M4 讨论验证后的文件三模式联动、安装整合与是否更换旧插件，不因已有管理界面自动切换。

**判断标准：**解决了什么痛点、减少哪些步骤、复用了什么、有没有新障碍。拒绝重复 SSO、企业 DLP、重写文件引擎、大型通用 RPA、重复任务数据库和过度审批。

参考：[个人/企业场景](PRODUCT_POSITIONING_AND_USE_CASES.md)、[混合 Profile/SSO](PROFILE_AUTH_AND_COLLABORATION.md)、[通讯/Agent](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)、[CloudFile 迁移](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。
