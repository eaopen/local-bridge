# Chat Source 多来源接入 V1：微信 / 网页会话 / 计划中的共生网络

状态：**Bridge 侧多来源抽象已实现并通过自动化测试；Soul 与共生网络的真实 Connector 均未交付**。日期：2026-10-30。目标分支：`dev`。

> 本文说明「帮我聊」如何从「只接微信」变成**可插拔的多个聊天来源**，以及新增来源需要满足的契约。
> 本次改动**不包含**任何供应商的登录、协议或消息实现；没有读取或发送真实聊天。验证记录见 [本地报告](validation/chat-source-multi-provider-20261030.md)。

## 0. 交付了什么

- `chat_source` 从「写死微信」改为 **provider 参数化**：每次读取由用户选择的本地 MCP Provider 提供，Bridge 只做契约校验与降级。
- 新增来源 **`web_page`** 与能力位 **`web_page_read`**：用于「用户已登录网页 / 浏览器会话中可见片段」的只读场景。
- 桌面「帮我聊」的片段面板新增**聊天来源选择器**：微信（本机 Provider）、Soul 网页会话（实验）、共生网络（计划中，当前禁用）。来源、能力位与按钮由 Provider 自报能力门控。
- 微信仍优先 `structured_db`（稳定会话 ID），其次 `local_index`、`desktop_ui`；网页来源不提供会话列表，不能批量枚举会话标题。
- 桌面设置新增 Soul 网页 Provider 的**实验性注册入口**；明确说明 Bridge 不随附该 Provider。
- `chat_copilot` 的上下文来源从固定白名单改为**受限的 `<provider>_<source>` 模式**，为后续 Connector 预留，不把来源字符串当指令。
- **未改**：Chrome 扩展、Native Host、inbox、任务中心、Agent 调度、CloudFile；微信现有 `local_index / desktop_ui / structured_db` 行为保持不变。

## 1. 来源模型

Provider 产出的每次读取都属于一种**完整度语义**，不是供应商名：

| source | 语义 | 门控能力位 | 备注 |
|---|---|---|---|
| `local_index` | 已有本地索引，非实时 | `local_index_read` | 可回退为缓存，需标注 `cached_fallback` |
| `desktop_ui` | 已打开界面的可见范围 | `live_read_available` | 必须声明 `live_read:true` |
| `structured_db` | 本机结构化库，稳定会话 ID | `structured_db_read` | 必须 `id_kind=stable_conversation_id` 与 `source_identity` |
| `web_page` | 网页/浏览器会话中用户可见片段 | `web_page_read` | 必须 `live_read:true`；输出 `session_visible_only:true`、`coverage_complete:false`、`id_kind=display_name` |

上下文来源字符串统一为 **`<provider>_<source>`**，例如 `wechat_local_index`、`soul_web_page`。`chat_copilot` 只接受：
`manual`，或 `<provider>_<kind>` 且 `kind` 属于上表、`provider` 为 ≤64 位可打印标识。来源字符串只作为模型输入中的**不可信数据字段**，不构成权限或工具调用。

## 2. Provider 必须实现的 MCP 契约

Provider 是用户自行安装的本地 stdio MCP 程序；Bridge 通过通用 `mcp_provider` 注册与调用它，工具名固定为 `<id>_status / <id>_list_chats / <id>_get_messages`。

```jsonc
// <id>_status -> result.structuredContent
{"read_contract_version":1,
 "capabilities":{"local_index_read":false,"structured_db_read":false,
                 "live_read_available":false,"web_page_read":true},
 "adapter":{"platform":"…","blocked_reason":null}}

// <id>_get_messages 参数
{"chat_id":"<用户指定/可见的会话名>","limit":1,"source":"web_page"}

// 响应
{"chat_id":"<与请求一致>","source":"web_page","read_status":"live","live_read":true,
 "messages":[{"text":"…","sender":"对方","sent_at":"…","source":"web_page","content_kind":"text"}],
 "coverage_complete":false}
```

Bridge 的硬校验（`chat_source::normalize`）：
- `chat_id` 必须原样回显；`source` 必须与用户选择一致；
- 消息数不超过请求的 1–20 条；总文本 ≤32,000 字节；不含 `\0`；
- `web_page` 必须 `live_read:true`，否则不采用片段；
- 非文本消息用 `content_kind:"non_text"` 表示，Bridge 跳过并显式上报 `skipped_non_text`；
- 任何失败只返回错误，**不自动重试、不切换 Provider、不伪造完成**。

## 3. Bridge / 桌面行为

- `desktop_chat_source_status/conversations/read` 三个命令新增可选参数 `provider`，**缺省仍为 `wechat`**，旧 UI 调用不受影响。
- `conversations/read` 先确认 Provider 已注册，再检查读取合约与来源能力位；未注册不会误报“需要升级”。
- UI 在一次检查/读取开始时快照 Provider；等待期间禁用选择器，返回后若选择已变化则丢弃结果。应用片段时使用后端返回的 `data.provider`，避免跨 Provider 误标。
- `status` 对未注册 Provider 返回 `{provider, registered:false}`；UI 显示「尚未安装或注册该聊天来源 Provider；仍可手动粘贴」。
- 能力位驱动来源下拉：Provider 未声明的来源禁用；没有任何可读来源时，读取按钮保持禁用。
- `web_page` 不要求会话列表：UI 隐藏「查找/选择聊天」，提示用户填写**浏览器中可见的会话名称**；不自动翻页、不后台监听、不订阅。`conversations` 也明确拒绝 `web_page`，并先检查 Provider 已注册与能力位，避免经 IPC/CLI 绕过 UI 枚举会话。
- `desktop_ui`/`web_page` 等「实时」来源若 Provider 返回 `read_status:"cached_fallback"`，Bridge 拒绝该片段，不把缓存冒充实时。
- 结果标签如实区分：来源是否实时、是否可见范围、是否 `cached_fallback`、跳过了多少非文本；网页来源额外提示「仅当前可见内容，可能有遗漏」。
- 选入片段只写入本轮输入；点击「生成建议」才调用现有 Agent；**没有发送到任何聊天软件的动作**。

## 4. 新增一个聊天来源需要做什么

1. 新建独立 Provider 仓库（不在 Bridge core 里写供应商逻辑），实现 §2 的三个工具与能力位；Provider id 建议 ≤51 字符（`<id>_get_messages` 仍受 MCP 工具名长度限制）。Provider 自己管理登录、数据、发送回执与平台条款。
2. 用户通过 CLI 注册：`local-bridge provider add <id> /abs/provider serve`；或在设置页为已知固定参数来源（当前是微信、Soul 网页）填写可执行文件。
3. UI 的来源映射表（`ui/chat-source.mjs` 的 `PROVIDERS`）增加 `<id>` 与其允许的来源集合、默认来源、友好名称；这是**唯一需要的 UI 小改动**。
4. 若提供新完整度语义（例如官方 API 可读到完整历史），不要冒充 `web_page`；应评审新增来源 kind 与能力位，再扩展 Bridge 校验。

## 5. 共生网络（计划接入）

- UI 预留「共生网络（计划中，暂无 Connector）」占位，**当前禁用**，避免误以为可读。
- 尚未定义其 Provider id 与来源类别（预留 `cobeing`）。若共生网络提供**官方 Room/消息 API**（例如持有邀请令牌的 HTTP 接口），它更接近「由 Provider 声明完整度的 API 来源」，不应直接套用 `web_page`；需要在 P2 单独评审并可能新增来源 kind。
- 本轮**不新建**记忆中心、身份层、社交网络或同步服务；不为共生网络预造数据库或账号体系。CoBeing/心舱的长期记忆仍不由 Bridge 承担。

## 6. 安全不变量（本轮全部保持）

1. 只在用户显式点击后读取；打开「帮我聊」不会自动读取任何来源。
2. Provider 由本机用户显式安装/注册；Bridge 不下载可执行文件、不存账号密码、不代登录。
3. 不提取浏览器 Cookie/Token，不直连第三方私有协议，不绕过验证码/风控。
4. 无自动发送、无后台监听/轮询、无批量枚举；`send_enabled:false`。
5. 片段在本轮内存中流转；A 路径不写 task_center、不写 localStorage、不写 inbox。
6. 网页/模型内容不能触发读取或发送；读取命令只能来自桌面 UI/用户 CLI。
7. 失败/超时/来源不匹配一律降级为可见错误，不自动重试或换 Provider。

## 7. 本次验证

- `cargo +1.90.0 test --workspace`：92 项通过（local-bridge lib 76、bin 3、native_process 2、core 11）。
- 严格 Clippy：`cargo +1.90.0 clippy --workspace --all-targets -- -D warnings` 通过。
- `cargo +1.90.0 fmt --all --check` 通过。
- 桌面：`cargo check/test --manifest-path crates/local-bridge-desktop/Cargo.toml` 通过（2 项）。
- Node：`node --test crates/local-bridge-desktop/ui/*.test.mjs` 23 项通过（含多 Provider 来源、双能力时微信优先 stable DB、能力门控、拒绝跨 Provider 来源、绝无 send/run 调用）。

**未验证 / 未交付**：
- 真实第二个 Provider（Soul 网页或共生网络）的端到端读取；本轮只用伪造 JSON 与 UI mock 验证契约与门控。
- Provider 切换竞态的并发复现；当前以快照 + 禁用选择器 + 返回后校验覆盖，未做真实窗口并发测试。
- 原生 Tauri 窗口的人工点击与视觉验收；Windows 实机。
- 共生网络的接口、授权与来源类别；Soul Provider 是否存在合法只读路径（见 [Soul 接入评估](SOUL_CHAT_INTEGRATION_V1.md)）。

## 8. 代码位置

- 核心：`crates/local-bridge/src/chat_source.rs`、`chat_copilot.rs`、`mcp_provider.rs`
- 桌面：`crates/local-bridge-desktop/src/main.rs`
- UI：`crates/local-bridge-desktop/ui/index.html`、`chat-source.mjs`、`chat-copilot.mjs`、`settings.mjs`、`app.mjs`
- 测试：`chat_source.rs` 单测、`chat-source.test.mjs`、`chat-copilot.test.mjs`、桌面 `main.rs` 单测
