# Chat Source 多来源本地验收（2026-10-30）

范围：`dev` 上的多来源切片。**只改 Bridge 与桌面 UI；没有安装/登录任何聊天软件或 Provider，没有读取或发送真实消息。**

## 已实现

- `chat_source` provider 参数化：`status/conversations/read` 接受用户选择的 Provider；缺省仍为 `wechat`，旧 UI 行为不变。
- 新增来源 `web_page` 与能力位 `web_page_read`；`normalize` 校验实时读取、跳过非文本、输出 `session_visible_only:true`、`coverage_complete:false`、`id_kind:"display_name"`。
- 上下文来源从固定白名单改为受限的 `<provider>_<source>` 模式；`manual` 与三个 `wechat_*` 仍通过，`soul_web_page`、`cobeing_web_page` 可被接受，未知 kind 拒绝。
- 桌面 UI：聊天来源选择器（微信 / Soul 网页实验 / 共生网络计划中禁用）、来源下拉按能力位启用、网页来源改为手动填写可见会话名（不列会话）、结果标签与错误降级、读取按钮在无可读能力时禁用；Provider 选择器在检查/读取期间禁用，片段归属使用后端返回的 `data.provider`。
- 微信默认来源优先级恢复为 `structured_db`（稳定 ID）→ `local_index` → `desktop_ui`；双能力时不再退回显示名来源。
- `conversations/read` 先确认 Provider 已注册，再检查合约与能力位；`conversations` 移除 `web_page`；`web_page` 返回 `cached_fallback` 或逐条 `chat_id` 不匹配时拒绝；Provider id 增加 `"."`/`".."` 与 51 字符上限（`<id>_get_messages` 工具名限制）。
- 设置页新增 Soul 网页 Provider 实验入口；明确不随附 Provider、不提取 Cookie/Token、不自动发送。Provider 固定启动参数新增 `soul => serve`。
- Chrome 扩展、Native Host、inbox、任务中心、Agent 调度、CloudFile 零改动。

## 自动化证据

| 检查 | 结果 |
|---|---|
| `cargo +1.90.0 test --workspace` | 92 项通过（local-bridge lib 76、bin 3、native_process 2、core 11） |
| `cargo +1.90.0 clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo +1.90.0 fmt --all --check` | 通过 |
| 桌面 `cargo check/test` | 通过（2 项：固定参数含 soul；provider 缺省 wechat） |
| `node --test crates/local-bridge-desktop/ui/*.test.mjs` | 23 项通过 |

新增/覆盖的关键用例：

- Rust `only_bounded_explicit_reads_are_valid`：`wechat/local_index` 与 `soul/web_page` 合法；非法 provider、`web_page` 以外的未知来源、越界条数拒绝。
- Rust `status_view_reports_provider_and_web_page_capability`：Provider 回显与 `web_page_read` 能力位。
- Rust `web_page_read_requires_declared_capability`：未声明能力/未声明合约时拒绝读取。
- Rust `web_page_read_marks_visible_only_and_skips_non_text`：`live_read` 必填、跳过非文本、`visible_only`、`coverage_complete=false`、错来源拒绝。
- Rust `multi_provider_context_sources_are_bounded_labels`：`soul_web_page` / `cobeing_web_page` 接受；裸 `web_page`、未知 kind、超长/非法字符拒绝。
- Rust `permission_denial_has_actionable_message`：微信与通用 Provider 的错误文案不再串味；微信 `structured_db` 文案保持原样。
- Rust `web_page_providers_cannot_enumerate_conversations`：`web_page` 不可列会话。
- Rust `unknown_provider_or_source_is_rejected`：`"."`/`".."`、超长 Provider id、非法条数拒绝。
- Rust `web_page_read_marks_visible_only_and_skips_non_text`：新增缓存冒充实时、逐条会话不匹配拒绝。
- Node `wechat prefers the stable structured DB when several sources are usable`：双能力默认来源。
- Node `a second provider keeps its own source and rejects cross-provider use`：`soul_web_page` 映射正确，微信/网页来源不互相冒用。
- Node `web page provider reads a named visible session without listing or sending`：只传 `provider/chatId/source/limit`，不加载会话列表，无 send/run 调用。
- Node `a provider without the declared capability cannot be read`：能力缺失时读取按钮禁用、列表区隐藏且不发读取命令。
- Node `source labels distinguish multiple chat providers without claiming delivery`：来源标签如实区分。

## 未验证

- 真实第二 Provider 的 stdio 端到端读取（无 Soul/共生网络 Provider 可用）；本轮仅伪造 JSON 与 UI mock。
- Provider 切换竞态的并发复现；当前靠快照、禁用选择器与返回后校验，未做真实窗口并发测试。
- 原生 Tauri 窗口的人工点击、浅/深色与窄窗口视觉检查。
- Windows 实机；真实 WebView2 行为。
- Soul 网页 Provider 的合法只读路径与真实账号验收（见 [Soul 接入评估](../SOUL_CHAT_INTEGRATION_V1.md)）。
- 共生网络的 Provider id、接口/授权与来源类别；当前只有禁用占位，未实现任何读取。

## 复核命令

```sh
cargo +1.90.0 fmt --all --check
cargo +1.90.0 test --workspace
cargo +1.90.0 clippy --workspace --all-targets -- -D warnings
cargo +1.90.0 check --manifest-path crates/local-bridge-desktop/Cargo.toml
cargo +1.90.0 test --manifest-path crates/local-bridge-desktop/Cargo.toml
node --test crates/local-bridge-desktop/ui/*.test.mjs
```
