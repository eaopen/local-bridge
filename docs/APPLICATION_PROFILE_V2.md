# ApplicationProfile V2 — 企业 Web 本地能力合同（设计稿）

**状态：规范草案（2026-10-28），不是已实现的 V2 解析器/在线执行。** 仅规范企业 Web → 本地 MCP/Agent 的声明和调用；不替换 [ApplicationProfile V1](APPLICATION_PROFILE_V1.md)。通用实现与验收见 [#6](https://github.com/eaopen/local-bridge/issues/6)。

## 1. 版本分离与兼容

- `schema_version=1`：保留原有文件客户端及既有 ApplicationProfile V1，现有 `/profile/v1`、Native Host、工作副本、Journal、Guard 均不更改；V2 **不自动迁移**既有任务/安装/绑定。
- `schema_version=2`：另一个**受信服务端只读元数据合同**，由企业应用提供。V2 可采用独立 `profile_id` 表示新企业 Web 应用实例，避免与正在运行的 V1 客户端实例冲突。企业应用可以同时发布 V1、V2，分别授权与测试。
- **浏览器协议与 Profile 版本不同**：既有通用 Chrome 扩展的 `local-tool-bridge:web:v1` 和 Native Messaging framed V1 与 V2 Profile 并不等价。Web V2 调用需要单独协商/版本标识，不移除已有 `context.share/summarize/relay.return/enterprise.pi.run`。Chrome 插件、Native Host 的独立版本与升级不强制绑定到 V2 Profile。
- V2 复用 V1 的 `profile_id`、`provider`、`environment`、`revision`、`expires_at`、`application`、`origin`、`services`、`channels`、`update` 的通用语义；`schema_version=2` 允许两个新增对象 `agent_policy` 与 `enterprise_ai_api`。**V1 解析器仍拒绝 V2 新字段**，不能对 V1 端点新增它们。

## 2. 工作/个人混合 Profile：统一 Runtime，个人 Provider 本地持有

`mixed_context` 是服务端**请求/提示**，不是个人 Profile 的配置，也不意味着企业应用获得个人工具读取/发消息权限：

```json
"mixed_context": {
  "mode": "mixed",
  "personal_capability_requests": ["mcp.tool.call", "communication.local"],
  "suggested_mcp_providers": ["wechat"]
}
```

- `mode=mixed`（默认）允许在**同一 OS 用户的 Runtime**中同时出现企业 Web AI、已有 Codex/WorkBuddy/Pi、用户注册的个人 MCP/通讯 Provider。无需切换企业/个人账号，也不在企业端保存微信账号、MCP 可执行路径或本地 Token。
- **Local PersonalProfile** 归本机用户；V2 通过 `personal_capability_requests` 申请用途，`suggested_mcp_providers` 只用于 UX 提示：`wechat` 可安装可不安装，**不会自动注册、启动或授予读取/发送能力**。
- 运行时 `EffectiveProfile = trusted ApplicationProfile V2 请求 ∩ 本机真实 Provider 状态 ∩ 本机用户许可 ∩ 目标业务对象授权`。本机 Provider 的注册、`tools/list`、登录、访问范围由其所属 Provider 决定；**配置已存在不等于已验证 ready**，能力显示与调用许可分离。
- `mode=enterprise_only` 是企业应用显式提出不使用个人工具的场景，必须同时清空 `personal_capability_requests`；**不改变已有本机个人 Provider 的配置或安装**。
- 个人与工作混合不额外创造一个 Bridge DLP 或重复 SSO；Chat/微信/文件的消息回执和权限仍由原服务掌握。企业任务主动调用个人微信需先获得当前操作用户许可与具体 Provider/工具的执行授权。网页/模型的自然语言文本不是同意或操作许可。

**已实现的最小代码**：`crates/local-bridge-core/src/profile_v2.rs` 独立类型/结构验证、纯函数 Provider 选择和 Personal MCP 权限交集；CLI `validate-profile-v2` / `profile-v2 inspect` 只能显示结构与本机已注册 Provider，`trusted=false`、`authorized=false`，**不是已交付的网页→微信或 Agent 运行授权**。V1 解析与 CloudFile 保持不变。

## 2. Agent 策略（可声明，不是本地授权）

```json
"agent_policy": {
  "mode": "first_ready",
  "preferred_providers": ["codex", "pi"],
  "allow_user_choice": true
}
```

- `preferred_providers` 是**当前应用声明的有序 Provider ID**，例如企业可根据需求设为 `["workbuddy","codex","pi"]`。**Local Bridge 不内建任何厂商优先级**。别的应用可按自己的需要设置或省略此对象。
- `first_ready`：**在任务实际开始前**逐一检查已安装、当前用户已登录/授权、真实调用接口已就绪、任务所需能力符合约束；只选第一个真正可用的 Provider。实验 harness/安装目录/Host 握手不能等同于 `api_ready`。
- 用户可以显式指定一个实际可用且已授权的 Provider（`allow_user_choice=true`），此时不被服务器建议顺序覆盖；本机用户许可始终优先于公开 Profile 声明。
- **任务提交后不跨 Provider 自动重试**：`accepted` 不代表 `completed`；已执行过工具、响应失败或结果未知不得擅自切到下一位重新执行。任务状态与审批仍由原 Provider 提供，Bridge 仅关联状态与可选结果摘要。
- Pi 的 `ready` 需要真实可用模型配置或下面的企业模型会话，不能把已安装但没有可用模型的 Pi 当作有效 fallback。

## 3. 企业 AI API（按需为 Pi 等 Agent 供给推理）

```json
"enterprise_ai_api": {
  "enabled": false,
  "protocol": "openai-chat-completions",
  "api_path": "/api/local-ai/v2",
  "grant_path": "/api/local-ai/v2/grant",
  "auth": "scoped_ticket",
  "stream": true
}
```

- `enabled=false` 表示**已声明但尚不可用**。企业应用实际部署的同源兼容 API、发票据服务、用户权限与 HTTPS 入口真实验证后才可发布 `enabled=true`。
- `api_path` 是当前授权企业 Web `origin` 下的 **API base path**，供既有 Chrome `enterprise.pi.run` 类能力调用其 `/chat/completions`；`grant_path` 是用户已登录应用签发短期、限用途/身份/模型范围票据的接口。路径来自应用实际部署，可因环境不同而不同，不用企业私有 URL 编译入 Bridge。
- 模型标识/短期票据由**已登录的企业应用在每次任务前**按当前用户和范围返回，不能放入公开 V2 Profile、Chrome 持久存储、本机 Agent 全局设置或日志。不得使用长期模型 API Key 作为浏览器可见票据。
- 企业模型服务拥有推理、授权、资源配额及费用；Bridge 只是本地 Agent/调用适配，**不开发第二套 9Router/模型网关**。一个现有企业 **业务对话 SSE** 并不自动符合 `openai-chat-completions`，需要服务端真正提供对应协议；不兼容时必须保持 `enabled=false`。
- 浏览器独立站点授权、Native Host 对 origin/路径/token/提示长度的限制、真正完成回执均仍生效。**企业 AI API 只提供模型，不自动向 Pi 开放 Shell、文件编辑或 MCP 工具**。对 AI 任务的 MCP/Agent 执行权限另按 V2 能力合同授权。

## 4. Web 本地能力（P0）及分层

- 企业 Web 只通过**受信通用浏览器扩展**接入本机能力，最小 Web V2 操作类型：能力发现、受控单次只读 MCP/Agent 任务、真实结果/失败状态。能力 ID 可使用 `mcp.tool.call` / `ai.agent.run`，**最终命名须由 #6 的协议实现与测试确定**。
- `channels[].capabilities` 仅表示应用请求，真正执行需同时满足源 Origin、浏览器逐站批准、本地 Provider 的 installed/ready、当前用户许可和能力自身的业务对象终判。不能因 Profile 声明就开放任意本机命令。
- **第一阶段只有 Web**，Bridge 可无独立 UI 或仅有最小设置；第二阶段可选通用桌面 Web 套壳，通过相同 V2 本地能力合同；第三阶段在 V2 稳定后由文件系统独立评估 V1 迁移，绝不预先切换 CloudFile 原链路。
- 缺少浏览器扩展、Bridge、Agent、企业模型或被撤销授权时，企业原 Web 功能正常；现有 CloudFile 文件链路继续 V1。

## 5. 需要实现才能声称 V2 可用

1. Local Bridge 增加 V2 只读解析/可信绑定及 V2 Web 能力调用，不改变 V1，测试两个版本并存；**当前只是草案**。
2. 企业 EAP/应用按照 V2 模型单独提供只读 Profile 与授权模型 API，实际 API 兼容及端到端身份必须验证；不存在的 API 不广告为 enabled。
3. Chrome 通过 V2 协商按 Origin 授权调用一个白名单只读 MCP 工具和一个真实可用 Agent，返回 `completed` 或真实的 `failed/unknown`，不能把传输接收当执行成功。
4. WorkBuddy 正式本地 Adapter 和 Codex/Pi 的真实 ready 检测分别验证，不能把已实测的独立 worker/harness 称为通用已上线。
5. DEV/PROD、原 CloudFile V1、现有业务页面及登录各自独立回归。**不为 V2 提前修改 CloudFile 或任何业务 AI 引擎。**

通用离线示例：[application-profile-v2.json](../examples/application-profile-v2.json)。**示例不是正式在线配置，也没有通过尚不存在的 V2 运行时验证。**

## 6. Web V2 status-only source implementation (2026-10-28)

The first Web V2 implementation uses the existing generic Chrome extension and independent Native Host; CloudFile V1 file operations are not routed through it.

- `local-bridge web-v2 grant-status <exact-origin> <registered-provider> <status-tool>` is an **explicit local OS-user grant** for a fixed no-argument tool named `status` or `*_status`. `revoke-status` removes the grant, `capabilities <origin>` shows the local exact-site grants. A remote Profile cannot grant itself an executable/provider.
- Chrome V2, only after **separate local MCP website permission**, supports `local.capabilities` and `mcp.status` via `local-tool-bridge:web:v2`. The extension obtains the exact origin from Chrome's actual sender; the Host requires a second exact CLI grant. The only MCP call is the locally registered, no-argument status tool.
- Personal WeChat Provider is **optional**; only `wechat_status` can participate in this first browser slice, not `wechat_list_chats`, `wechat_get_messages`, `wechat_send_message`, or arbitrary tools. Status-only response does not imply WeChat logged in or an AI Agent run is possible.
- V2 `profile-v2 inspect` remains offline, unauthenticated declaration/registration inspection (`trusted:false`, `activated:false`, `authorized:false`). Future trusted enrollment, full Web Agent dispatch, business-object grants, desktop shell and V1 CloudFile migration remain distinct tasks.
- Chrome branch: [Chrome V2 draft PR](https://github.com/eaopen/local-bridge-chrome-extension/pull/4). This source path has Rust CI / Chrome Node test coverage but **not real Chrome + Native Host + Etech SSO + signed-in WeChat acceptance**.
