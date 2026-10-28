# Agent 验证基线 — Codex、WorkBuddy 与 Pi (2026-10-28)

**产品决策：Codex 与 WorkBuddy 都必须进入 Local Bridge 的 P0 Agent 验证矩阵；但两者始终是可选安装和可选调用的 Provider。** 无论用户只装一个、两个都不装，Local Bridge 的基本 MCP、Chrome 用户交接、CloudFile 现役文件主链都不应失效。Pi 是可选备选，现已补充本机 RPC 真实任务验收，不增加运行时必装要求。

> 本机最新进展：2026-10-28 已通过桌面原账号 WBIPC + 内置 CLI/ACP 的真实文件任务，以及 Bridge Rust 实验入口独立核验。详见文末“WBIPC + 内置 CLI/ACP 本机验收”；早期未安装设备记录为历史记录。正式通用 Adapter 尚未交付。

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

## B. WorkBuddy 方向与外部真实证据要分开测试

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

### B3. WorkBuddy Desktop 已有登录 → 内置 CLI / WBIPC 原账号真实任务（新 P0 实验）

**本机验收现已通过，见文末完整记录；以下为外部实现参考。** [Workbuddian 2.6.14 的 2026-10-09 验收](https://github.com/jiang198012/workbuddian/blob/main/docs/issue-10-zero-setup-2026-10-09.md) 记录 macOS / WorkBuddy 5.7.7：在正常 `session_hello_ack` 后取得已注册 `wb.request/http.fetch`，以它代理桌面宿主已有账号的认证模型 HTTP 请求，启动 WorkBuddy **安装包自带 CLI** 的独立 worker/ACP 会话，Obsidian 插件真实收到准确 Agent 输出；并经人工批准完成文件写入与 MCP。没有抽取/解密云端 Token、没有另行登录独立 CLI。

- `http.fetch` **本身仍不是任务创建接口**。真实任务由自有内置 CLI worker 执行，ACP 接收完成/审批事件，代理仅提供官方桌面登录态支持的模型请求；详见 [Broker](https://github.com/jiang198012/workbuddian/blob/main/src/providers/codebuddy/workbuddyBroker.ts)、[Native](https://github.com/jiang198012/workbuddian/blob/main/src/providers/codebuddy/workbuddyNative.ts)。
- 第三方记录的正常 WBIPC `http.fetch` 在该版本每次约 50 秒、64 万字节（请求/响应）、完整缓冲非流式；Windows 真机、长任务、计费归属及生产稳定性仍待验证。协议未公开，厂商升级可能破坏；不把它作为官方稳定支持。
- 此方法可以优先验证 Local Bridge Web → Native Messaging → WorkBuddy 自有 CLI Agent 单次运行，也可复用 CLI 已支持的 Skill/MCP，但**不能**宣称复用了 GUI 原有会话、专家界面或所有桌面功能。
- 本仓的新执行任务定义与分阶段真机验收详见 [Issue #22](https://github.com/eaopen/local-bridge/issues/22)；不要把“客户端 IPC 方法已列出”标记为 run 完成。

### B4. CodeBuddy Code CLI 独立登录 + 同账号共享积分（保守备选）

官方计费资料明确 CodeBuddy / WorkBuddy 同一账号积分共享，因此 CLI **独立完成登录**不等于再次购买订阅。Draft [PR #21](https://github.com/eaopen/local-bridge/pull/21) 添加 `local-bridge agent codebuddy <prompt|->` 一次性只读结果调用，已有跨平台编译与模拟结果解析测试；还没有在 WorkBuddy 实际安装机的真实模型调用证据。此通路不复用桌面登录、Skills、对话或 GUI agent；不能与 B3 混算。

## C. 两者共同的最小验收

| 检查 | 完成条件 |
| --- | --- |
| 发现 | 已安装 / 未安装明确区分；不要求两者都安装 |
| 授权 | 正确区分登录成功、第三方 Scope 缺失、接口不可用 |
| 单次运行 | 只对真实已授权调用，记录 `started/failed/completed/unknown`；不伪造输出 |
| MCP | 只有来源明确许可的已注册工具可被调用，模型/网页文本不自动获得任意写/发信权限 |
| 结果交接 | 把任务引用和已确认结果交回发起方；不建设第二套 Agent 聊天记录 |
| 缺失/失败 | 任一 Agent 不可用，仍可使用另一个或 Web AI **人工**接力；不自动跳过用户授权或绕过配额 |

**执行顺序：**先在安装 WorkBuddy 的目标设备完成 B3（内置 CLI/ACP 执行真实短任务 + Bridge 返回结果）；B1 WorkBuddy 消费 Bridge MCP 可并行；B4 CodeBuddy 单独登录为低依赖备选。若已有第三方应用获批相应 Scope，再验证官方 B2；GUI 自动化仅在上述通路不能操作 WorkBuddy 特定桌面功能时考虑。Codex 已有适配保持不受 WorkBuddy 阻塞。

> 状态标记：`Codex native CLI run = 已实测`；`Codex App Server protocol + run = 已实测，但 Bridge Adapter 未交付`；`WorkBuddy 原桌面账号实验 = 本机通过，见文末`；`WorkBuddy MCP Connector with Bridge = 未验证`。

## WorkBuddy CLI 增量（2026-10-28）

Bridge → WorkBuddy 官方 Local Assistant 的 `status/send/messages` 已实现，详见 [CLI 使用与限制](WORKBUDDY_CLI.md)。此前未实现的记录为历史基线；当前已提供 CLI 适配，但 OAuth 实机调用仍待验收。客户端 UI 短任务成功不代表 API 授权或此适配器端到端成功。

## 本机 WorkBuddy Desktop GUI 实测（2026-10-28 13:35–13:36）

**GUI 发任务、执行文件操作、读取完成状态及结果的可行性已通过；独立 Bridge GUI Adapter 尚未实现。** 本节是另一台已安装 WorkBuddy 的本机实测，前文未安装设备记录不能作为本机当前状态。

- 环境：WorkBuddy Desktop 5.9.3、Hy3，沿用当前桌面登录和原有权限设置；没有再次登录、第三方 OAuth、读取或复制认证凭证。
- 实际链路：测试请求经真实 Native Host `context.share` 写入现有 Inbox，再由真实 `local-bridge inbox read` 读回；Codex CUA 将该请求粘贴到新任务并发送一次；WorkBuddy 读取专用 CSV，计算汇总并创建 `result.json`；外部验证器读取文件独立计算校验。
- 输入四笔虚构订单；排除 cancelled 后，实际输出为 3 笔、10 件、98.30 总额、39.80 待付款，所有字段和唯一任务标记逐项匹配。预期结果没有放进提示词，结果文件由 WorkBuddy 创建。
- UI 明确显示“已完成 18s”；回复包含标记和结果路径。离开会话再重新打开“验收GUI任务并写入结果文件”，仍能读取相同标记、状态和汇总。发送被接受或文件存在均未单独用作成功依据。
- [验收回执](validation/workbuddy-gui-20261028/receipt.json)、[请求](validation/workbuddy-gui-20261028/request.json)、[输入 CSV](validation/workbuddy-gui-20261028/orders.csv)、[实际输出](validation/workbuddy-gui-20261028/result.json)。测试目录 `/tmp/local-bridge-wb-gui-f16176dd10` 保留。

### 落地边界

本次 GUI 驱动是 Codex 的 CUA，不是 Local Bridge 二进制自身；所以证明了可实现的桌面任务闭环，不能标记 `bridge_integrated=true`。当前 OAuth CLI 也不能代替这一场景。正式接入还需可由 Bridge 调用的桌面驱动，并处理当前用户桌面/辅助功能权限、串行操作、运行/审批/失败/超时状态。未知结果不得自动重发，应先按唯一标记找回原任务。

本次只验证单任务成功及会话重新定位，没有验证锁屏、后台无人值守、并发、多版本、弹窗审批或故障恢复。AX 元素编号和自动生成的任务标题不稳定，不宜持久化为任务主键；应使用请求唯一标记和限定输出路径关联结果。文件结果采用 JSON 校验，避免把桌面富文本拆字当作精确结构化数据。提示词中的文件范围是任务要求，不是已验证的强制文件隔离机制。

## WBIPC + 内置 CLI/ACP 本机验收（2026-10-28）

**已通过：Bridge Rust 实验入口 → WorkBuddy 桌面原账号请求代理 → 内置 CLI 独立 worker → ACP 文件任务完成 → Rust 独立文件核验。** 不再将 GUI 视为唯一可行路径。此次本机 WorkBuddy Desktop 5.9.3 已登录，未执行 CLI 登录、第三方 OAuth 或提取云端凭据。读取的 WBIPC discovery ticket 只用于本机正式握手；临时 loopback token 由实验进程随机生成，云端认证由桌面宿主代理。

### 实测证据

1. 正常 WBIPC 双向 HMAC 握手，以 `local-bridge-validation` 身份取得已注册的 `wb.request/http.fetch`；通过该通道读取当前账号模型目录。
2. 使用 WorkBuddy 安装包内 Electron Node runtime 和 `cli/dist/codebuddy-lite-wb.mjs`。5.9.3 实测单独 `--acp` 在初始化前退出；加 `--serve --port 0 --host 127.0.0.1 --acp --acp-transport stdio` 后通过。HTTP worker 使用随机密码，不开放无认证服务。
3. 文本任务回复精确为 `WB_BRIDGE_AGENT_OK_20261028`，ACP `stopReason=end_turn`；[文本回执](validation/workbuddy-native-20261028/text-receipt.json)。
4. 文件任务使用 `Read,Write` 工具集合、空 MCP 和独立临时工作目录/config；没有 `-y` 或 bypassPermissions。收到 `session/request_permission` 后，仅当工具为 Write、路径为本轮新测试文件、内容精确为目标字符串时选择 `allow_once`。ACP 请求 ID 为 0 也必须处理。第一轮客户端漏处理该编号，文件未生成；退出该 worker 后在新目录修正重测通过。
5. 最终由 `cargo +1.90.0 run --example workbuddy_native_validation` 实际启动任务：worker PID 43411，session `01a1810f-6335-7247-8efb-5d950ccb96be`。收到工具 `completed`、ACP `end_turn` 和实际路径；Rust 独立读取 25 字节精确匹配 `LOCAL_BRIDGE_WORKBUDDY_OK`。worker 退出码 0。[完整 Bridge 回执](validation/workbuddy-native-20261028/bridge-receipt.json)、[Rust 核验](validation/workbuddy-native-20261028/rust-verification.json)、[文件产物](validation/workbuddy-native-20261028/bridge-workbuddy-test.txt)。

### 交付与限制

- 实验代码：[Rust 验收入口](../crates/local-bridge/examples/workbuddy_native_validation.rs)、[可选 Node 测试辅助程序](../scripts/workbuddy-native-validation/run.mjs)。Node 24+ 仅为主动运行该实验所需，不加入 Bridge 核心启动依赖。Rust example clippy 和 diff 检查通过。
- 本次并非 PR #21 的独立 CodeBuddy CLI：该 Draft 分支已 fetch 检查，它默认禁用工具且需要自身登录。本机现有未提交修改未被覆盖，未合并 PR。
- `http.fetch` 不是 Agent 任务 RPC，但可为内置 CLI 提供已认证模型请求通道；此前据方法列表断言不能执行 Agent，结论应修正。成功的模型请求 `/v2/chat/completions` 均为 HTTP 200。
- 启动时 `/v3/config` 返回 400，临时配置还出现无缓存提示；本次两类任务仍成功。这是版本兼容问题，不应视为完整协议兼容。上游报告约 50 秒/640 KiB/缓冲响应，本机未压测这些边界。
- 已验证的是独立 worker，不是 GUI 既有会话/专家/工作空间的控制；Skills/MCP 继承、Windows、取消/断线/并发、额度扣费差值均未验收。
- 目前是固定任务的实验 harness，**不是已发布的通用 `agent workbuddy` 原账号 Adapter，也没有 Web/Chrome 接线**。正式实现需保留工具审批、请求关联、未知结果不重放和自有 worker 清理。
- 源码参考：[Workbuddian Broker](https://github.com/jiang198012/workbuddian/blob/main/src/providers/codebuddy/workbuddyBroker.ts)、[Native worker](https://github.com/jiang198012/workbuddian/blob/main/src/providers/codebuddy/workbuddyNative.ts)、[模型目录](https://github.com/jiang198012/workbuddian/blob/main/src/providers/codebuddy/workbuddyModels.ts)。Broker/model helper 源自其 MIT 代码，保留许可；本机以真实结果作为验收依据。

## Pi RPC 本机验收与三 Agent 汇总（2026-10-28）

**Pi 1.3.0 真实文本/文件任务已通过；成功配置为本机已有的 `pi-9router-ext` + `9router/m3cn/MiniMax-M2.7`。** 认证由 Pi 和已有扩展管理，没有复制凭据或更改全局默认模型。

| Provider | 真实任务证据 | Bridge 接线层级 | 尚未证明 |
| --- | --- | --- | --- |
| Codex | 前序 CLI + App Server 短任务 completed | 正式现有 CLI/Native Host 路径；App Server 单独实测 | App Server Adapter、跨平台同等验收 |
| WorkBuddy | 本机原桌面账号 WBIPC + 内置 CLI/ACP：文本、单文件写入、Rust 读取通过 | 固定任务 Rust 实验入口 | 通用正式 Adapter、GUI 会话/专家继承、Web 接线 |
| Pi | 本机 RPC：文本、write 工具、agent_settled、Rust 读取通过 | 固定任务 Rust 实验入口 | 通用正式 Adapter、Pi MCP→Bridge、其他模型及跨平台 |

三个方案都已有真实 Agent 执行证据；不是“三个通用正式 Adapter 已全部交付”。Codex 记录沿用前序实测，本次没有重新执行 Codex。

### Pi 结果与失败分层

- `/opt/homebrew/bin/pi --version` 返回 `1.3.0`。Bridge Rust 验收进程启动 `pi --mode rpc --no-session`，先 `get_state`，然后发送唯一 ID 的 `prompt`；持续消费 JSONL，检查最终 assistant 的 `stopReason=stop`、`agent_end` 和 `agent_settled(aborted=false)`。不能只凭 prompt accepted 或 agent_end 成功。
- 默认 `minimax/MiniMax-M2.7`：`pi auth check` 显示 ready，但真实请求返回 `401 invalid api key`。验收入口正确报失败，没有启动后续文件任务。[失败回执](validation/pi-20261028/default-provider-failure.json)。未修复或覆盖该默认配置。
- 9router 属于用户现有扩展。禁用所有扩展时会报 Unknown provider，因此测试显式加载这一已安装扩展，保留其他扩展自动发现关闭；`auth check` 的 invalid_state 不能代替该扩展的真实请求结果。
- 成功文本会话 `01a18122-3f50-74fb-b861-fb9a83e22998`，worker PID 50257，输出精确匹配本轮唯一标记。[文本回执](validation/pi-20261028/text-receipt.json)。
- 成功文件会话 `01a18122-5615-73c4-965b-b25edd71638b`，worker PID 50323，`write` 工具 `isError=false`，最终返回实际路径；Rust 独立读取 `bridge-pi-test.txt`，18 字节精确匹配 `LOCAL_BRIDGE_PI_OK`。两次 worker 均正常退出。[文件回执](validation/pi-20261028/file-receipt.json)、[产物](validation/pi-20261028/bridge-pi-test.txt)。
- 测试使用新临时目录、无持久会话、关闭 MCP/Skills/提示模板/项目上下文；文本禁用全部工具，文件任务仅开放 write。Pi 此次没有交互审批流程，文件范围由测试任务约束，并非操作系统文件隔离；不能直接作为网页任意文件写入接口。

### 复现

[验收源代码](../crates/local-bridge/examples/pi_validation.rs)，不增加第三方 Rust 依赖，不修改全局 Pi 设置。默认运行测试现有默认模型；可显式指定本机已有模型/扩展：

```sh
LOCAL_BRIDGE_PI_VALIDATION_MODEL=9router/m3cn/MiniMax-M2.7 \
LOCAL_BRIDGE_PI_VALIDATION_EXTENSION="$HOME/.pi/agent/npm/node_modules/pi-9router-ext/src/index.ts" \
cargo +1.90.0 run --example pi_validation
```

每次创建新测试目录；运行会实际调用模型并创建一个测试文件。未通过文本任务时停止，不自动选择模型或重放写任务。Rust example 的 clippy 与 diff 检查通过。协议以本机 1.3.0 随包文档及 [Pi v1.3.0 RPC 文档](https://github.com/earendil-works/pi/blob/v1.3.0/packages/coding-agent/docs/rpc.md) 为准。
