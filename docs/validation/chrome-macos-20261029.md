# Local Bridge 真实 macOS 浏览器 / Codex / Tauri DEV 验收（2026-10-29）

## 证据范围

平台：macOS ARM64；Chrome for Testing 156（独立用户数据目录）；Local Tool Bridge Chrome 0.6.2 加 [可选 Agent 参数修复 #7](https://github.com/eaopen/local-bridge-chrome-extension/pull/7)；Local Bridge CLI/Native 与可选 Tauri 来自 [开发预览 CI #40751395991](https://github.com/eaopen/local-bridge/actions/runs/40751395991) 的 Mac 合格产物（该初始 CI 的 Windows 打包失败，后续 [#40752156726](https://github.com/eaopen/local-bridge/actions/runs/40752156726) 已使 **Mac/Windows 双平台产物成功**）。本验收使用**隔离的测试扩展副本、Chrome 数据目录、合成网页与临时 Mock MCP**。不等于真实企业 SSO / CloudFile 文件工作流或 Chrome Store 正式发布。

1. **CLI**：`status`、`self-test`、`native-host status`、`tasks list` 在隔离 HOME 返回真实结果；未注册 Provider/Chrome 时明确报告未安装而非伪成功。
2. **MCP stdio**：本机 Node Mock 显式注册为 `bridge-test`，`tools/list` 发现 `bridge_test_status`，`tools/call` 回传精确标记 `LB_DEV_STATUS_20261029`；没有联网企业数据。
3. **实际 Chrome Native Messaging**：Chrome for Testing 的扩展真实 ID 产生后，安装独立 `com.eaopen.browser_bridge` Manifest；浏览器 `host.status` 返回 `connected_to_local_bridge:true`。用户触发 `context.share` 返回 `stored:true`，原 Bridge Inbox 能查到同一合成任务。
4. **Web V2 MCP**：本机授权精确测试 Origin 的 `bridge_test_status`，Chrome content script → Service Worker → Native Host → Bridge MCP Client → Mock Provider，真实网页收到原样标记。撤销插件站点 MCP 权限后下一次调用明确返回 `site_mcp_permission_required`。
5. **真实 Codex**：该 Mac 用户原 Codex CLI `login status` 确认已登录，无密码/Token 导出或复制。仅为合成测试 Origin 暂时授予 `codex`，用 Chrome Web V2 发送一条只要求返回固定标记、无文件操作的短任务。第一次 `accepted`，同一 `client_key` 的第二次返回**原 `task_id` + `reused:true`**。随后 `agent.read` 观测到 `running` 及明确 `task.completed` / `terminal:true` / `completed`；原本机任务事件日志的精确文本匹配 `LOCAL_BRIDGE_CODEX_E2E_20261029`（无终态猜测、未新增第二个任务）。
6. **Tauri DEV .app**：从 unsigned artifact 启动，操作系统可见窗口枚举确认 `Local Bridge Desktop Dev` layer 0；共享 AgentPreferences 由同一隔离 HOME 下的 CLI 保存、读回并恢复默认。**尚未通过窗口内点击保存/重开回读的交互验收**。
7. **CloudFile 缺失边界**：`tasks list --cloudfile` 在未安装可用 Agent2 Provider 时回报 `not_configured_or_unavailable`、`complete:false`，而不声称零文件任务。Agent2 原数据根、Guard、Journal、Writer 均未被本次验收操作。

## 测试环境差异与清理

Chrome Stable 156 这次没有通过本机命令行测试参数成功装载 unpacked 插件，改用**Chrome for Testing**。它在指定自定义 user-data-dir 时实际从该测试目录的 `NativeMessagingHosts/` 检索本机 Host；常规用户 HOME 及 ChromeForTesting 通用目录中的测试 Manifest 均无法替代此路径。本事实只声明**本次测试安装/版本的观察**，不要硬编码成所有 Chrome 部署的通用规则。

测试用 extension manifest **仅在隔离副本里**预先加入 `nativeMessaging` 和 `http://127.0.0.1/*`，绕开无人值守情况下 Chrome 的权限弹窗。因此 Chrome 本机通讯/Origin 二次授权/撤权是真实的，但**普通用户手动点击可选权限弹窗仍未验收**。

清理：已撤销测试站点 Agent 授权，移除临时放入常用用户 Chrome 目录的两个**新 Bridge 专属** Manifest，恢复扩展测试副本原 manifest，停止测试 HTTP 服务/Tauri 窗口，并精确清除测试产生的真实用户 `LocalBridge` 目录（测试前不存在）。未触碰 `com.cloudfile.current_agent`、旧扩展或 CloudFile 文件目录。

## 仍需单独完成

- Windows 普通用户 Chrome + Native Host / 可选 Tauri 安装实机验收；目前 Windows 仅 CI 产物/编译通过。
- 正式扩展的逐站手工权限授予 / 撤销 UX；HTTP/SSE 中断切换 Native 的浏览器恢复演练，确保同 `clientKey` 不重复执行。
- Tauri 窗口内 Agent 配置保存、重启回读、任务筛选交互；已有后端真实读写，不等于前端 UI 验收。
- WorkBuddy/Pi 的 Web Agent E2E、真实企业 SSO/Profile V2 和业务 API。
- [CloudFile Agent2 Draft Provider #26](https://github.com/eaopen/cloudfile-local-agent2/pull/26) 尚未合并；必须在原 Agent2 **同一数据根**验证脱敏只读任务，而非新空目录。文件三模式仍由原 Agent2 独占。
