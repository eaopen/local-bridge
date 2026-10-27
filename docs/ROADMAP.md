# Roadmap — 优先交付用户可用的跨系统协作

**产品定位（2026-10-27）：** 面向个人和企业的轻量级本地能力连接与任务协作平台。**默认混合、按需隔离**。每个开发任务都要证明减少用户操作，而不是增加一个平台。

## M0 — 当前已实现的基线

Rust 0.1.0：只读 MCP stdio（bridge.status、cloudfile.status）、CloudFile 组件状态查询、旧 Channel 与 ApplicationProfile V1 的离线校验。**尚未实现**可信在线 Profile 注册、PersonalProfile 运行时、通用任务聚合、实际 Agent run、双向通讯 Adapter 和 CloudFile 文件操作 IPC。CloudFile 文件编辑仍走现有 Agent2/Hub/Server。

## M1 — 最短 Web / 本机调用闭环（优先）

- 可信 ApplicationProfile + LocalBinding + 能力发现，明确 declared/installed/available/authorized；个人工作空间只做轻量本地配置，不创建云账号或服务端 PersonalProfile Schema。
- 在已登录的企业 Web 经原应用后端短期操作授权、Chrome/Native Messaging 完成一个只读 MCP 工具的真实调用和返回。不新建 SSO；Bridge 不在线时原 Web 正常。
- CloudFile Dev/Prod Host、Journal、凭据与更新来源不互相覆盖。

## M2 — 个人与企业同机工作（首个真实产品成果）

- 一次实际授权的本地 Codex/WorkBuddy 或可选 Pi Agent run；不把安装状态冒充 API 可用。
- CloudFile Agent2 既有任务摘要旁路接入 BridgeTask，文件本身继续由现役引擎处理。
- 同一台公司电脑同时运行企业文件任务和个人聊天助手任务；不切换 Profile、不增加跨 Profile 审批，统一展示真实结果。

## M3 — 通讯双向连接 + 企业 Web AI 增强

- 使用一种**确实可用且获用户许可**的第三方/官方通讯 Connector，完成指定聊天消息读取/新消息订阅 → Agent → 草稿或授权发送 → 发送状态查询。微信、Soul 是场景，不预设其官方个人聊天 API。
- Etech 元枢复用原 Web、SSO 与 AI API，仅添加通用 MCP/Agent 本地能力。不把企业代码或固定地址放进 Bridge 核心。
- 可选的个人/企业隔离设置仅在真实需求下追加，不作为默认连接的 P0 阻断。

## M4 — 验证后才考虑统一桌面入口

- CloudFile 查看、普通编辑、独占编辑通过最小 IPC 适配逐项真实验收；旧 Host、Guard、Journal 和活动文件任务不迁移。
- Windows 普通用户优先，Mac/Linux 同合同逐步覆盖；Chrome/Agent 升级与回退须保持已有生产使用习惯。
- 完整桌面壳、更多扩展、复杂任务 UI 在实测价值明确后建设，不强制 WebView/Pi/特权后台。

**判断标准：**解决了什么痛点、减少哪些步骤、复用了什么、有没有新障碍。拒绝重复 SSO、企业 DLP、重写文件引擎、大型通用 RPA、重复任务数据库和过度审批。

参考：[个人/企业场景](PRODUCT_POSITIONING_AND_USE_CASES.md)、[混合 Profile/SSO](PROFILE_AUTH_AND_COLLABORATION.md)、[通讯/Agent](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)、[CloudFile 迁移](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。
