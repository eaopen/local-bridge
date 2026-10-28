# Agent 验证基线 — Codex 与 WorkBuddy (2026-10-28)

**产品决策：Codex 与 WorkBuddy 都必须进入 Local Bridge 的 P0 Agent 验证矩阵；但两者始终是可选安装和可选调用的 Provider。** 无论用户只装一个、两个都不装，Local Bridge 的基本 MCP、Chrome 用户交接、CloudFile 现役文件主链都不应失效。Pi 是后续备选，不阻塞前两者验证。

这是**实测进度和下一次验收标准**，不是宣称已经交付双 AgentAdapter。要区分五层：

1. **installed**：本机真有可用客户端/CLI，不等于登录完成；
2. **authenticated**：用户已登录/第三方应用已获授权，不等于接口可用；
3. **api_ready**：真实调用官方本地/远程接口能收到有效结果；
4. **run_completed**：一段无文件修改的虚构短任务真正完成，有明确完成事件/回执；
5. **bridge_integrated**：通过 Local Bridge 启动/返回状态和结果；不凭厂商单独运行冒充已集成。

## 实机验证记录

| 项目 | Codex | WorkBuddy |
| --- | --- | --- |
| 当前可访问设备 | macOS ARM64 开发 Mac | 同一台 Mac |
| 安装/CLI 发现 | `codex-cli 0.147.0`，已安装 | 未发现 WorkBuddy.app / WorkBuddy CLI |
| 账户/授权 | `codex login status` 显示已登录 ChatGPT | **无法验证**，不是全局不支持 |
| 官方接口 | `codex app-server --listen stdio://`，实际 `initialize` 和 `account/read` 成功 | 已核对官方 Open API，但没有可用第三方应用授权 |
| 实际短任务 | `thread/start → turn/start → item/completed → turn/completed`；虚构测试串返回预期值，最终 `completed` | **未执行，不可标记通过** |
| Local Bridge 目前代码 | `agent codex` 使用现有 `codex exec` 临时只读任务；Native Host → Codex CLI 也已用虚构数据实测成功 | **没有 Bridge→WorkBuddy AgentAdapter** |
| 结论 | Codex 本机接口 + 任务均已验证；**App Server 还没有接入 Bridge 代码** | WorkBuddy 官方路径存在，**仍需在已安装且授权的机器验证** |

本轮只处理虚构测试文本，未读取实际企业文件、聊天记录或浏览器 Cookie；App Server 实测以只读沙箱与不执行工具为前提。Codex 任务成功不代表 Windows 机器、浏览器点击、所有模型额度和写文件审批也已验收。

## A. Codex 继续实现的最小闭环

官方协议：[Codex App Server](https://developers.openai.com/codex/app-server/)。

- P0 可用：保留现有 `codex exec --ephemeral --sandbox read-only`，用户**主动**从 CLI 或 Chrome Popup 启动；处理 `turn.completed` 与错误，不把启动成功当执行完成。
- P0 下一步：在真正需要持续会话/状态时，**复用官方 App Server stdio**：`initialize`、`account/read`、`thread/start`、`turn/start`、`turn/completed`，将本机既有登录和会话信息继续交给 Codex 自己管理。不得在 Bridge 重写聊天数据库。
- 实测条件：检测 CLI、授权、接口；使用无副作用虚构任务，至少返回一个 `completed`，同时验证缺失客户端、未登录、超时/失败和中断不会显示成功。工作目录、沙箱与运行审批必须遵循用户当前明确授权。
- 结果属于 Codex；Bridge 只保存必要引用与当前状态。出现结果未知时不要换 Agent 自动重试。

## B. WorkBuddy 两个方向要分开测试

官方文档：[Local Assistant Open API](https://open.workbuddy.cn/docs/openapi) · [Connector](https://open.workbuddy.cn/docs/connector) · [第三方应用权限](https://open.workbuddy.cn/docs/third-party-app)。

### B1. WorkBuddy → Local Bridge（WorkBuddy 消费本机 MCP）

WorkBuddy Connector 官方支持 **本地 stdio MCP + Skill**；对用户已安装/有权使用的 WorkBuddy，先用原有 Local Bridge `local-bridge mcp` 提供的只读 `bridge.status` 工具做一次验证：

1. 真实 WorkBuddy 客户端已安装、登录并具备添加本地 Connector 的权限；
2. 按官方连接器格式登记 **本机** `local-bridge mcp`；需要发布市场审核时按 WorkBuddy 实际规则办理，不擅自绕过；
3. 在 WorkBuddy 中主动请求读取 Bridge 状态，收到真实工具结果；测试断开 Bridge 后显示不可用。

**通过 B1 只证明 WorkBuddy 可以使用 Bridge 的 MCP，不能证明 Bridge 可以反向发起 WorkBuddy Agent run。**

### B2. Local Bridge → WorkBuddy（Bridge 主动委派 Agent）

官方 WorkBuddy 本地助理 Open API 采用 HTTPS/OAuth 2.1，必须先有获批的第三方应用和用户授权：

- `user.localassistant.readable`：`GET /openapi/v2/localassistant` 查询在线及有权限的会话历史；
- `user.localassistant.invokable`：`POST /openapi/v2/localassistant/message` 才能主动发送测试任务；
- 用户授权后再根据接口实际返回的任务/历史数据跟踪状态；**消息提交/接受不等于 Agent 已完成任务**；
- 本机/Chrome/开发示例不保存或嵌入 `clientSecret`；遵守官方应用类型、审核、Scope 与用户授权要求。

当前可访问 Mac 没有 WorkBuddy 程序，也没有在本次验证中取得对应第三方应用 OAuth 权限，因此 **B1 与 B2 均未真实通过**。即使其他机器已安装 WorkBuddy，也要在该设备验证登录和实际接口，不能凭本机软件名称推断有 API 授权。

WorkBuddy Share/Deep Link 可作为日后**用户手工接力**渠道，但它需要独立的应用注册/签名/版本要求，且只是填充用户输入框，不等于自动 Agent API。现阶段不要为它新增一套分享框架或未获许可的桌面 UI 自动化。

## C. 两者共同的最小验收

| 检查 | 完成条件 |
| --- | --- |
| 发现 | 已安装 / 未安装明确区分；不要求两者都安装 |
| 授权 | 正确区分登录成功、第三方 Scope 缺失、接口不可用 |
| 单次运行 | 只对真实已授权调用，记录 `started/failed/completed/unknown`；不伪造输出 |
| MCP | 只有来源明确许可的已注册工具可被调用，模型/网页文本不自动获得任意写/发信权限 |
| 结果交接 | 把任务引用和已确认结果交回发起方；不建设第二套 Agent 聊天记录 |
| 缺失/失败 | 任一 Agent 不可用，仍可使用另一个或 Web AI **人工**接力；不自动跳过用户授权或绕过配额 |

**执行顺序：**先在有 WorkBuddy 的实际设备完成 B1；若第三方应用/Scope 已获批准，再做 B2。Codex App Server 可在当前已验证设备继续做最小 Adapter，Chrome/企业 Web 多通道联动仍需单独验收。不要把 WorkBuddy 审核等待作为整个 Local Bridge 或 Chrome 插件开发的阻断项。

> 状态标记：`Codex native CLI run = 已实测`；`Codex App Server protocol + run = 已实测，但 Bridge Adapter 未交付`；`WorkBuddy Client/API run = 未验证`；`WorkBuddy MCP Connector with Bridge = 未验证`。
