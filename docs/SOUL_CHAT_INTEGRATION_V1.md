# Soul 网页会话接入「帮我聊」评估（V1）

状态：**Soul 专用 Connector 未实现；与 Soul 解耦的 Bridge 多来源抽象已实现**（2026-10-30，见 [多来源接入 V1](CHAT_SOURCE_MULTI_PROVIDER_V1.md) 与 [本地验收](validation/chat-source-multi-provider-20261030.md)）。日期：2026-10-30。目标分支：`dev`。

> 回答「能否像微信一样把 [home.soulapp.cn/chat](https://home.soulapp.cn/chat) 接进 Local Bridge「帮我聊」」。
> 本次**未实现 Soul 接入、未使用真实 Soul 账号、未发送任何消息**；只读分析了公开 SPA 资源、未登录页面与官方协议。Soul 只是「网页会话来源」的一个候选，Bridge 侧不写 Soul 专属逻辑。
> 标注：**【已验证】**＝本次直接读代码/抓包/渲染确认；**【推断】**＝由公开代码或结构推出、未在登录态复现；**【未知】**＝需要真实账号、Soul 授权或真机。

## 0. 结论

1. **微信能被读取，靠的是「可选 MCP Provider + Bridge 通用适配 + 桌面 UI」；Soul 缺的不是 UI/Agent，而是一个合法、稳定、可自动化的读取来源。**
2. **Soul 没有面向第三方的聊天 API/开放平台**：官网只有营销页，`open/developers/openapi.soulapp.cn` 不可达，`mp-doc.soulapp.cn` 为非公开小程序文档。聊天能力只通过官方 App/PC 网页版提供。**【已验证/截至本日未发现】**
3. **自建协议客户端不可行也不可做**：网页 IM 的 PC 协议 v16 对 `sauth`/`command` 帧加密（加固 ESM `/vendor/single-entry.preview.reinforced.esm.js`），并用 token 算 `crc`、`mixTimestamp`；HTTP 同源鉴权走 Cookie（无 `Authorization`、无 CORS）。复刻这些等于逆向加密，协议也明文禁止。**【已验证·静态代码】**
4. **平台明确禁止机器人/自动化**：用户协议写有「未经我们授权……不得通过任何机器人、蜘蛛等程序或设备进行操作」「不得通过反向工程、反编译、反汇编……」，商业性使用还需书面授权。**【已验证】**
5. **平台能识别 AI 辅助/自动回复**：消息 `extMap.isAiAssistedChat==='1'`、`msgType=32 && messageType==='auto_reply'` 被排除在「对方已回复」之外；配合 24h/30s 礼仪锁、friendly/社交分配额，自动代聊不但违规，产品上也会被标记。**【已验证·静态代码】**
6. **唯一技术上可行的形态是「用户已登录官方网页 + 浏览器扩展按次读取片段」**（优先读页面 localStorage 里的会话缓存，DOM 兜底），经 Native Host 交给 Bridge 生成草稿，用户在 Soul 里手动发送。但它与现有 Chrome 扩展「不做 DOM/页面级抓取」的规则冲突，且需要新增一条 `inbox → chat_source` 数据通路，**不能作为本轮默认交付**。**【已验证规则冲突；技术面推断】**
7. 推荐：**个人价值走今天就能用的手动粘贴**；工程上已完成与 Soul 解耦的 `web_page` 来源抽象（不承诺 Soul 可用）；Soul 连接器作为 **P2 决策项**，先解决三件事——扩展范围/单独立项、ToS 与隐私评估、真实账号只读验收。**不先写协议客户端再补授权。** 共生网络同样按「计划接入来源」处理，当前只有 Provider/Source 插槽，没有 Connector。

## 1. 先看微信是怎么接进来的（P1 多来源改造前）

> 本节记录 2026-10-30 多来源改造前的微信链路；改造后的 provider 参数、`web_page` 来源与 UI 选择见 §5-P1 与 [多来源接入 V1](CHAT_SOURCE_MULTI_PROVIDER_V1.md)。

### 1.1 真实链路（自上而下）

| 层 | 位置 | 作用 |
|---|---|---|
| UI | `crates/local-bridge-desktop/ui/index.html:60-110`、`ui/chat-source.mjs`、`ui/chat-copilot.mjs` | 「帮我聊」页 + 可折叠片段面板：检查来源、选会话、勾选片段、放入本轮输入 |
| Tauri 命令 | `crates/local-bridge-desktop/src/main.rs:145-159`（注册 `:180-182`） | `desktop_chat_source_status / conversations / read`，**改造前参数里没有 provider** |
| Bridge 适配 | `crates/local-bridge/src/chat_source.rs:42-115`、`normalize:116-183` | 调 Provider、校验来源/条数/身份、失败降级 |
| MCP Client | `crates/local-bridge/src/mcp_provider.rs` | **通用** stdio JSON-RPC：显式注册、`tools/list`、`tools/call`；每次调用新起进程（`:351-371`），90s 超时（`:22`）、单行 1MB（`:21`）、工具 ≤128（`:314`） |
| Provider | 独立 [wechat-provider](https://github.com/eaopen/wechat-provider) | 六工具：`wechat_status / list_chats / get_messages / search / send_message / get_send_status` |
| Agent | `crates/local-bridge/src/chat_copilot.rs`、`agent_options` | 只把用户勾选片段装配为输入，复用现有 Codex/WorkBuddy/Pi；不新建模型路由 |

### 1.2 哪些是通用的，哪些是微信专用

- **可直接复用**：`mcp_provider` 的注册/握手/调用与超时约束、capability 门控思路（Provider 自报能力，UI 才亮按钮）、`chat_copilot` 的单槽内存会话、Agent 选择与授权边界。
- **微信硬编码（第二来源必须处理）**：
  - `chat_source.rs:40` 固定 `call("wechat", ...)`；`:45` 固定查 `id=="wechat"`；`:49/66/111` 固定 `wechat_*` 工具名。
  - `:52-57` 只上报 `local_index_read / structured_db_read / live_read_available / live_read_verified`，没有网页会话能力位。
  - `:60/88` 来源固定 `local_index / desktop_ui / structured_db`；`:18/20/99/105/108` 错误文案全是「微信」「完全磁盘访问权限」。
  - `chat_copilot.rs:49-55` 上下文来源白名单只接受 `manual` 与三个 `wechat_*`。
  - `desktop/src/main.rs:115-121` `provider_args` 只允许 `wechat`/`cloudfile`；UI 设置页、`settings.mjs`、`app.mjs` 各有一处微信入口。

## 2. Soul 网页版实测证据（2026-10-30）

### 2.1 入口、登录与会话

- 未登录 `https://home.soulapp.cn/chat`：HTTP 返回 200 的 SPA 空壳（服务端不 30x），**前端路由重定向到 `/login`**；页面是扫码 + 手机号登录，含《用户协议》《隐私政策》与二维码 120 秒倒计时。**【已验证】**
- 手机号短信前有**网易易盾滑块/安全验证**（`/yidun-captcha.js`，错误码 `10100` 需滑块、`10099` 需安全验证）；扫码需手机 Soul App 确认。自动化登录必然撞验证码。**【已验证】**
- 鉴权：HTTP 同源 `https://home.soulapp.cn/pc/*`，`fetch(...,{credentials:'include'})` 走 **Cookie**，代码中无 `Authorization` 头；登录后的 IM token 只在页面内存（`imToken`，缺失时调 `/pc/login/new/token`）。`soul_auth_token` 在代码中只被删除、未发现写入。**【已验证·代码】**
- 请求强制设备头 `os: PC`、`app-id: 10000010`、`app-version/version: 1.0.0`、`channel-id: official`、`device-id`（页面随机 UUID）；缺头返回 `10002`，补齐后未登录返回 `20001`（登录失效）。**【已验证·实测】**
- **无 CORS**：扩展 origin 发 `OPTIONS` 得到 200 但没有 `Access-Control-Allow-Origin`；扩展不能从自己的 origin 直连 HTTP API，只能在页面同源上下文里读。**【已验证】**
- 网页版 2026-10 上线，与手机端消息实时同步（新闻来源）。**【已验证·新闻】**

### 2.2 HTTP 端点（页面自用，不是公开接口）

- 统一响应 `{code,message,data,success}`：`10001` 成功、`10002` 通用失败、`20001` 登录失效、`20002` 方法不支持、`10010` 请先开通 PC 会员、`10099/10100` 需安全验证/滑块。**【已验证·代码+实测】**
- 聊天相关（只读）：`/pc/chat/session/resume`（会话列表）、`/pc/chat/session/users/sync`、`/pc/chat/session/ext`、`/pc/chat/message/resume`、`/pc/chat/card`、`/pc/chat/v`（IM 协议版本）、`/pc/chat/limit/friendly/check`、`/pc/chat/limit/socialGraceScore`、`/pc/teenager/config`。
- 写/发送相关：`/pc/chat/expression/send`、`/pc/chat/image/send`、`/pc/chat/image/upload-token`、`/pc/chat/voice/transcribe`，以及关注/举报等。
- 以上只说明**页面自己怎么取数**；第三方复用要带用户 Cookie，属于「用程序操作 Soul 服务」。**【已验证端点；合规判断见 §2.4】**

### 2.3 IM WebSocket 与消息模型

- `wss://webim.soulapp.cn:8663/ws`，未登录也能完成 `101 Switching Protocols`，认证在应用层 `sauth`。**【已验证】**
- 帧格式 `[version(1B), msgType(1B)]`；`MsgType: ping=0, sauth=1, command=2`；心跳 `ping` 5s/10s，断线重连最多 10 次。**【已验证·代码】**
- **v16 加密**：PC 端 `configureCrypto` 后，`sauth`/`command` 的 protobuf body 经 `encryptBody()` 加密（帧头 `EncryptFlag.TRANSPORT_DES=1`），加密实现来自加固 ESM；`command` 还 `buildEncryptedUserId`、`mixTimestamp`、`updateCrc({token,cmdId,tsSec})`。**自建客户端必须复刻加固加密与 crc，等同逆向。**【已验证·代码】**
- 消息模型：`msgType` `0` 文本、`1` 图片、`4` 语音、`7` 表情、`8` 撤回、`11/12` 猜拳/骰子、`32` 通用（礼物、`auto_reply` 等）；文本在 `msgCommand.textMsg.text`；`cmdId` 必有，接收侧 `msgCommand.timestamp` 可用，发送侧 v16 会混淆时间戳。**【已验证·代码】**
- 发送链：`sendMessageToConversation → ChatService.sendTextMessage → 本地 pending → WS command → RespCommand(confirmed)`，8 秒未确认即失败。**【已验证·代码】**

### 2.4 发送约束与风控（为什么不能代聊）

- **礼仪锁（客户端 localStorage）** `soul_im_etiquette_<me>:<peer>`：`peerHasSent` 为假且 24h 内发过 → 拒绝；`textSendLease` 30 秒独占；配合 `navigator.locks` 防并发。**【已验证·代码】**
- **服务端限制**：`/pc/chat/limit/friendly/check`（不友善行为/礼仪配额）、`/pc/chat/limit/socialGraceScore`（社交分）、内容风控（`risk-block-marker`）、图片审核、500 字上限、未成年人实名限制、PC 会员限制（`10010`）。**【已验证·代码】**
- **AI 识别**：`peerHasSent` 判定排除 `isAiAssistedChat==='1'` 与 `auto_reply`；即平台**刻意不把 AI 辅助/自动回复当作真人回应**。对「帮我聊」是明确的强信号：平台不欢迎代聊，自动发送会被识别、被礼仪锁拦截，甚至限权。**【已验证·代码】**

### 2.5 合规红线（原文）

- 《Soul App 用户服务协议》（更新/生效 2025-12-30，[原文](https://themis.soulapp.cn/m/GFYf-6-DTTWO0x7OoErgbg.html)）：
  - 「1.4 您可以通过 Soul 官网及我们合法授权的渠道、途径、方式获取 Soul 产品与/或服务。」
  - 「1.6 您仅有权以合法合规的方式使用 Soul App，**不得以恶意破解等非法手段使用**……」
  - 「未经我们授权，任何人不得擅自使用（……**以及通过任何机器人、蜘蛛等程序或设备进行操作**等），亦不得……**通过反向工程、反编译、反汇编或其他类似行为**获得 Soul 产品与/或服务的源代码等……」
  - 「3.2.9 ……不得恶意删除、篡改、伪造、隐匿此类生成内容标识……」
  - 「3.2.15 我们授权您**非商业性使用**……如果要进行商业性使用……必须事先获得我们的书面授权和许可。」
  - 「通过其他任何未经我们合法授权的渠道、途径、方式获取的 Soul 产品与/或服务……我们将不承认其合法性。」
- 《Soul App 个人信息保护政策》（更新 2026-07-30，[原文](https://themis.soulapp.cn/m/OyraGhrcJdj-RVlMC5d06A.html)）：委托处理/共享/第三方 SDK 规则中**未授权任何第三方客户端读取用户聊天内容**。**【已验证】**
- 网页 IM SDK 还内置自动化/环境/完整性检测：`detectContainer/detectEngine/detectJsEngine/detectEnvironment/detectUaVersionLie/isTampered/auditCriticalApis/HOOK_FLAG_BITS`，经 `reportRiskData` 上报风控。**【已验证·存在；触发条件与处置未知】**

## 3. 为什么不能照搬微信

| 维度 | 微信（现状） | Soul（网页版） |
|---|---|---|
| 数据位置 | 本机微信数据/界面 | 只有登录态网页；Cookie 鉴权 + 内存 IM token |
| 接入方式 | 独立 Provider 读本机数据、RPA 发送（真实收发尚未验收） | 无官方 API；HTTP/WS 都要求用户凭据，WS v16 帧加密 |
| 平台态度 | 已知风险，仓库明确不绕安全登录 | 协议明文禁止机器人/蜘蛛自动化、反向工程；SDK 主动检测 |
| 发送代价 | 幂等键、`uncertain` 不重发、需人工确认 | 24h/30s 礼仪锁、服务端配额、内容风控、AI 回复识别 |
| 技术可行性 | 本机数据可读 | 自建协议客户端＝逆向加密；只剩页面内读取 |

一句话：微信 Provider 至少还有「用户本机数据」这个落脚点；Soul 第三方要读，只能**用程序操作 Soul 的服务或用户已登录的页面**。Bridge 自己的 `AGENTS.md` 写明：通讯适配器必须获用户许可，**不建私有协议绕过，不假设用户同意可以覆盖平台规则**。

## 4. 可选形态对比

| 形态 | 读取 | 发送 | 合规/规则 | 成本 | 判定 |
|---|---|---|---|---|---|
| **P0 手动粘贴**（已有） | 用户自己复制 | 用户自己发 | 无新增 | 0 | **立即使用** |
| **P1 Bridge `web_page` 来源抽象** | 不含真实数据源 | 无 | 无新增 | 小 | **已实现**（不承诺 Soul，见 [多来源文档](CHAT_SOURCE_MULTI_PROVIDER_V1.md)） |
| **P2-A 独立 MCP Provider（`soul-provider`）** | 需 Cookie/token 或逆向 WS | 无 | **违反协议**；WS 加密不可合法复刻 | 大 | **不做** |
| **P2-B 浏览器页面内按次读取**（扩展 + Native Host） | 仅用户可见/缓存片段 | 无（用户手动发） | 与扩展「不做 DOM/页面抓取」规则冲突；ToS 仍需评估 | 中 | **仅在产品/法务明确后另立项** |
| **自动化/代聊** | 全量 | 自动 | **明确不做** | — | **不做** |

### 4.1 为什么 P2-A（像微信那样做 Provider）不可行

- 没有官方接口；要读只能复用用户 Cookie（HTTP）或 IM token + v16 加密帧（WS）。
- 复刻加固加密、crc、时间戳混淆＝**反向工程**，协议明文禁止；风控还会检测自动化环境。
- 就算技术上打通，账号风控、封号、法律与道德风险都由用户承担，产品不应交付。
- 因此 **Bridge 侧即使把 `chat_source` 参数化，也不应发布一个 `soul` Provider**。

### 4.2 P2-B 的真实代价（为什么本轮不做）

技术上唯一可行：用户已登录官方网页 → 扩展在**页面同源**按次读取（优先 `soul_im_convlist_<uid>` / `soul_im_chatlist_<uid>` 缓存，DOM 兜底）→ Native Host → Bridge 片段 → Agent 只出草稿 → 用户复制回 Soul 手动发送。但：

1. **与扩展现有规则冲突**：`local-bridge-chrome-extension/AGENTS.md:7,12,14` 明确「不做 page-wide scraping / Web Chat DOM scraping / auto-send」，扩展单用途是「用户审阅后的上下文转交」。加聊天抓取需要扩 scope、重审商店用途与隐私政策。**【已验证】**
2. **需要第二条数据通路**：`chat_source` 只认 MCP Provider（`chat_source.rs:40`）；要接扩展内容，得新增 `inbox → chat_source` 或 Native Host → 片段通路，并避免把私聊原文落盘到 inbox（当前 `context.share` 会写盘，`inbox.rs:85-135`，与 P0「不默认保存私聊」冲突）。
3. **已读回执副作用**：Soul 页面在窗口可见 + 消息进视口时通过 `IntersectionObserver` 发送已读回执。打开会话并让消息进入视口，**可能让对方看到「已读」**，这会改变社交语义；只读缓存可降到最低，但缓存可能滞后（`chatlist` 主要在退登/同步时落盘）。**【已验证·代码】**
4. **缓存/DOM 均不稳定**：缓存键与结构随版本变化；DOM 依赖 `.message-flow/.bubble-row/.bubble-text` 等类名，改版即断。
5. **ToS/隐私仍需评估**：即使只在用户自己的浏览器里读自己的数据，仍属程序化操作 Soul 服务，且 `3.2.15` 限制商业化使用。

结论：**P2-B 是「可以做实验」而非「可以交付」的形态**；必须先由产品/法务确认扩展范围与合规边界，再由真实账号验收，不能在本轮顺手实现。

## 5. 推荐路线

### P0｜今天可用：手动粘贴（0 代码）

用户在 Soul 网页选中/复制一段对话 → 打开「帮我聊」→ 粘贴 → 选「帮我理解/帮我回应/帮我理清自己」→ 生成、编辑、复制回 Soul 手动发送。**不依赖 Provider，今天就有个人价值。**

### P1｜已实现的通用切片（与 Soul 解耦）

目标：让「帮我聊」具备**第二个来源的接入点**，但不承诺 Soul 可用、不访问 Soul、不改变微信行为。以下为实际落地内容，验收见 [本地报告](validation/chat-source-multi-provider-20261030.md)。

1. `crates/local-bridge/src/chat_source.rs`：`status/conversations/read` 增加 `provider` 参数；工具名 `format!("{provider}_{tool}")`；新增 `web_page` 来源与 `web_page_read` 能力位；`normalize` 增加 `web_page` 分支（要求 `live_read==true`、跳过非文本、输出 `session_visible_only:true`、`coverage_complete:false`、`id_kind:"display_name"`）；微信三个既有来源（`local_index/desktop_ui/structured_db`）的分支与错误文案保持不变。
2. `mcp_provider.rs`：仅 `identifier → pub(crate)`，协议零改动。
3. 桌面 `main.rs`：三个命令加 `Option<String> provider`（缺省 `wechat`）；`provider_args` 预留固定参数入口；设置页测试补一条。
4. UI：来源选择器（微信 / 网页会话）、`web_page` 选项按能力位启用、去 `wechat_` 前缀、标签与错误降级；`settings.mjs`/`app.mjs` 增加对应区块（只显示注册/工具状态，不存凭据）。
5. `chat_copilot.rs`：上下文来源从固定白名单改为受限的 `<provider>_<source>` 模式（仍拒绝未知 kind 与非法字符），为 Soul/共生网络等后续 Provider 预留。
6. 测试：新增网页来源能力门控/错来源/非文本/`visible_only`、多 Provider 上下文来源用例；现有 `chat_source`、`mcp_provider`、桌面未知 provider 拒绝、Node「绝无 send/run 调用」断言保持通过。

> 注意：P1 只解决「Bridge 能接第二个来源」，**不等于 Soul 能读**。Soul 的真实数据通路仍取决于 P2 的决策。

### P2｜Soul 连接器（决策项，不是默认排期）

只有同时满足以下条件才启动：

- 产品/法务确认：扩展范围可扩，或单独立项；只读、用户显式、不发送、不商业化使用；
- 数据面接受：**优先只读 localStorage 缓存、不打开会话**（避免已读回执）；DOM 仅在用户当前就停在会话页时兜底；
- 真实账号验收：登录后缓存键/字段、DOM 选择器、消息分页、已读回执、失败降级；
- 隐私：片段只在本机内存流转，用完即清；不写入 task_center/localStorage/inbox（如需写入必须单独告知并取得同意）。

**P2 仍不做**：自建 HTTP/WS 客户端、headless 登录、Cookie/token 注入、拦截/解密 v16 帧、自动发送/代聊、后台监听/轮询。

## 6. 关键词回答

- **「能不能像微信一样接？」** 不能。微信有本机数据落脚点；Soul 没有官方 API，自建协议客户端等于逆向加密，协议明文禁止。
- **「技术上能不能读？」** 能，但只能依附用户已登录的官方网页，由用户显式点击、按次读取可见/缓存片段；不是无人值守的 Provider。
- **「能不能自动发？」** 不做。Soul 有礼仪锁、配额、内容风控，并能识别 `isAiAssistedChat`/`auto_reply`；代聊既违规也可能被限权。
- **「最省事且合规的集成是什么？」** 手动粘贴（今天可用）+ 已落地的 Bridge 通用 `web_page` 来源抽象（不承诺 Soul）。Soul/共生网络专用连接器等授权与产品决策。

## 7. 验收与未验证

**无 Soul 账号即可做（CI）**
- P1 的 Rust/Node 单测与离线 stdio fixture Provider（伪造 `soul_*` JSON，验证能力门控、来源校验、失败降级、绝不触发 Agent/发送）；
- 回归：`cargo +1.90.0 fmt --all --check`、`cargo +1.90.0 test -p local-bridge --lib`、桌面 `cargo check/test`、`node --test crates/local-bridge-desktop/ui/*.test.mjs`。

**必须由真实登录环境确认（未满足不得声称支持 Soul）**
- 登录后 `/chat` 的真实 DOM/虚拟滚动/分页；
- `soul_im_convlist_*` / `soul_im_chatlist_*` 的实际键名、字段、写入时机与滞后；
- `/pc/chat/session/resume`、`message/resume` 的真实字段与上限；
- 已读回执的实际触发与对方可见性；
- 发送风控、礼仪配额、会员/未成年限制；
- 真实登录下 `/pc/chat/v` 是否为 16（决定 WS 是否必然加密）。

**本次未验证**
- 未登录任何账号、未进入真实聊天页、未发送消息、未调用写接口、未绕过验证码、未提取任何 cookie/token；
- Soul 是否有未公开的官方合作接口。

## 8. 参考

- 入口：[home.soulapp.cn/chat](https://home.soulapp.cn/chat)
- 《Soul App 用户服务协议》：<https://themis.soulapp.cn/m/GFYf-6-DTTWO0x7OoErgbg.html>
- 《Soul App 个人信息保护政策》：<https://themis.soulapp.cn/m/OyraGhrcJdj-RVlMC5d06A.html>
- 网页版上线报道：[砍柴网](https://news.ikanchai.com/2026/1014/669247.shtml)
- 仓库内相关合同：[P0 聊天助手方案](PERSONAL_CHAT_COPILOT_P0.md)、[可选 MCP Provider](LOCAL_MCP_PROVIDER_V1.md)、[通讯适配器](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)、[聊天片段验收](CHAT_SOURCE_VALIDATION_2026-10-30.md)
- 代码位置：`crates/local-bridge/src/chat_source.rs`、`crates/local-bridge/src/mcp_provider.rs`、`crates/local-bridge-desktop/src/main.rs`、`crates/local-bridge-desktop/ui/chat-source.mjs`
- 多来源架构与 Provider 契约：[Chat Source 多来源接入 V1](CHAT_SOURCE_MULTI_PROVIDER_V1.md)
- 本轮验收：[多来源本地报告](validation/chat-source-multi-provider-20261030.md)
- 扩展边界：`local-bridge-chrome-extension/AGENTS.md`、`docs/PROTOCOL_V1.md`
