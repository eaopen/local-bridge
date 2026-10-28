# Web V2 通信方案：Chrome 可信接入 + 可选本地直连（2026-10-28）

## 产品边界

- **Etech/元枢原入口不得依赖 Local Bridge**。现有 `/aiChatList`、浮窗 `aiRobot`、BPM 聊天、CloudFile V1 均保持不变。本地增强只能通过独立 `/aiChatLocalV2`（或日后显式功能启用）提供；插件/Bridge/WorkBuddy 缺失时直接展示原版元枢，无自动切换、无空白页。
- **Local Bridge 是通用本地能力服务**；Chrome 扩展独立存在。业务用户身份归 EAP，WorkBuddy 登录归 WorkBuddy，本机站点授权归 Local Bridge；Chrome 扩展**不是第二套企业 SSO**。
- **一个任务、一个事件记录、两种可选通信通道**；不因 Chrome 与 HTTP 传输同时存在而双开 Worker、双建任务。默认 V2 Agent 任务异步，`start` 只返回 `accepted/task_id`；`events(cursor)` 读取 `message.delta`、`message.final`、`tool.update`，直到独立的 `task.completed/failed/unknown`。

## 当前 P0：Chrome 辅助授权 → Bridge 本地 HTTP/SSE 主通信

**正常路径**：用户从 Chrome 插件对明确的企业 Origin 授权一次（单独授予 MCP/Agent）；Native Host 创建短期（10 分钟）的随机本机会话，只把受信 localhost 地址和临时凭据返回**当前页面**。之后页面直接 \`fetch\` Local Bridge HTTP API 启动任务，通过 HTTP SSE 持续收取多次 WorkBuddy 事件。Chrome 不再负责模型回复转发、长轮询或事件保存。Local Bridge 只监听 127.0.0.1，验证 Origin、Host、短期 Bearer、OS 用户精确授权，使用本机唯一任务记录。

**异常路径**：企业站点 HTTP/LNA/CORS/浏览器策略不允许访问 loopback，或本地监听服务没启动时，退回现有 Chrome Native Messaging \`agent.start/agent.poll\` 兼容模式；任务编号和 \`client_key\` 继续一致，禁止重复创建 WorkBuddy 任务。没有插件时完全回退原 Web 元枢。

**不要**把每个模型 token 当成一个独立 `sendNativeMessage()` 返回。该接口每次短请求会启动一个 Host 进程，并且只读取第一个响应；当前已采用 `agent.run` 返回 `task_id`，随后由页面 `agent.poll(task_id,cursor)` 分批获得多条消息。这个路径短、容易定位故障，也无需常驻桌面 UI。

元枢的新入口应采用**独立 `aiChatLocalV2`，包装完整原 `AiChatList`**，只有插件/Host/本机 WorkBuddy 能力确实可发现时才显示增强入口。不要向原 `aiChatList.vue` 和 `aiRobot.vue` 添加默认按钮，更不修改 BPM 会话实现。

## P1：本机 localhost HTTP / SSE 仅作为可选数据通道

当 P0 真实通过、且所在浏览器/站点满足安全前提时，可以让 **Bridge 本身**附加一个仅绑定 `127.0.0.1`（必要时 `::1`）的轻量本地 HTTP 服务。用户的 Web 元枢在授权后直接 `fetch` SSE 事件或查询状态；**Chrome 只承担发现、用户许可、可信会话启动/短期授权凭据交换与不支持时的回退**。不直接绕开授权、改成任何网页可调的裸端口。

防护约束：
1. 浏览器/企业侧必须是真实 HTTPS 安全上下文；请求本地 loopback 受浏览器的 Local Network Access/loopback permission 约束，CORS 也必须满足；**不通过关闭浏览器安全检查、全站放宽 CORS 或内网 HTTP 强行绕过**。Etech PROD 仓库所见 HTTP 地址只是配置示例，需现场确认真实 HTTPS 发布方式。
2. 本地服务只监听 loopback，验证 Host/Origin、拒绝 DNS rebinding、预检不执行任务、`Access-Control-Allow-Origin` 仅精确已批准站点、禁用通配 `*` 和跨站 cookie；每次任务额外验证短期、限 Origin/任务/操作的不可猜测凭据。
3. 短期凭据从本机受信 Chrome/Bridge 配对流程得到；**不能**由远程 Profile、网页自身声称的 Origin、端口号本身代替认证。涉及企业业务对象仍需 EAP 登录与业务权限；个人 Provider 需要本机用户授权。
4. SSE 建议用带 `Authorization` 的流式 `fetch()`，而非将长期 token 放 URL（浏览器原生 EventSource 不支持自定义 Authorization header）。只输出已有任务的事件，无通用 Shell/MCP 运行端点。
5. 原 Native Messaging `start/poll` 保留为 **HTTPS/LNA 不可用、企业旧浏览器、Local HTTP 连接失败**时的兜底。同一 `task_id/cursor` 可跨通道续读，不重新提交 WorkBuddy。

**P0 的 HTTP/SSE 已实现首版源码**：\`local-bridge web-v2 serve\` 启动独立 localhost HTTP 监听；Native Host \`web.v2.session.issue\` 按本机授权签发随机短期凭据，网页 \`GET /v2/capabilities\`、\`POST /v2/agents/tasks\`、\`GET /v2/agents/tasks/{id}/stream?cursor=...\`、\`POST /v2/mcp/status\` 直接调用 Bridge；Chrome 仅连接和故障回退。**尚未真实浏览器联调**。WebSocket 是以后确需双向工具审批/互动时的可选通道，当前 Agent 进度单向传输用 SSE 更小、更稳定，不需要创建并维护两套消息机制。

## 版本与验收

- CloudFile Profile V1、Agent2、老 Chrome/Host、文件三模式与 Journal/Guard 不动。
- EAP Profile V2、WorkBuddy 多事件合同和新版 Chrome 是独立开发分支/PR；不合并改变旧元枢默认使用。
- P0 验收：**未安装插件**、未安装 Bridge、无 WorkBuddy、WorkBuddy 未登录、站点未授权、Host 不可用时，打开新入口仍显示原元枢，旧 `/aiChatList` 无差异；可用时可独立提交任务并凭 cursor 接收至少两次回复、最终正确判定完成或失败。
- P1 验收：站点 HTTPS + loopback 许可成功时从 HTTP/SSE 续读同一任务；权限拒绝或连接断开回退 Native poll，绝不重复启动任务。

技术资料：Chrome Native Messaging 官方说明、Chrome Local Network Access 限制。详见 [Web V2 WorkBuddy](WEB_V2_AGENT.md)、[V2 Profile](APPLICATION_PROFILE_V2.md)。


## Web JS/TS Client V2（单一实现，静态版本引入）

Local Bridge 提供 [clients/web-v2](../clients/web-v2/README.md) 中的零外部依赖 `client.js` + `client.d.ts`（`2.0.0-preview.3`）。企业 Web 应用应在**构建时锁定、打包**已审查版本，按 `createLocalBridgeClient()` 调用 `connect / runAgent / readAgent / mcpStatus`。客户端统一管理 Chrome 首次授权、HTTP/SSE 主要传输、Chrome Native 后备传输与 `task_id/seq` 去重恢复；各业务 Web 不应再各自复制这些逻辑。

**不推荐**让网页通过 `<script src="http://127.0.0.1/...">` 动态执行本地 Bridge 下发的 JS：这会增加本地服务不可用时的启动依赖、静默版本变更、来源/CSP 与供应链风险。运行期只获取短期连接信息和受信的**JSON 能力数据**，绝不通过远程 Profile 下发可执行 JS。本地 Bridge SDK 以后可以独立发布 npm 包，但目前无需额外服务或发布系统。EAP Web 的首个接入通过源码哈希锁定静态拷贝，**原元枢入口不依赖 SDK**。

客户端测试（在 Local Bridge 工作树）：`node --test clients/web-v2/client.test.mjs`。


## 2026-10-28 定案：统一 Agent V2，loopback HTTP 默认不启 TLS

**Web SDK 仅导出 `runAgent/readAgent`，`api_version=2` 必须写入实际 JSON 请求；用户可选 `agent/model/thinkMode/agentMode`。** 旧 `startWorkbuddy/readWorkbuddy/pollWorkbuddy` 及 V2 vendor 专用 `web.v2.agent.start/poll`、`/v2/workbuddy/tasks` 路由已在开发版本直接删除。WorkBuddy 后台执行器保留，Codex/Pi 增加了独立 CLI 文本 Worker，共享 V2 授权、状态和任务游标。三者均未做企业网页/真实登录端到端验收；`eligible` 仅代表可尝试提交，不宣称 `ready`。Explicit `model/thinkMode` 若不能真正应用必须拒绝。

默认本机 127.0.0.1 **HTTP 非 TLS**，不配置证书、不静默安装根证书。Chrome 142 本地网络访问许可、145 `loopback-network`/ `local-network` 划分属于浏览器权限，获得 loopback 许可后安全上下文访问本机 HTTP 可用（具体仍受企业策略/CORS 影响）。不兼容时回退 Native Messaging。企业如独立部署可信 localhost HTTPS 代理可显式选择固定代理端点，非默认功能，不关闭浏览器 TLS 校验。详见 [统一 Web SDK](../clients/web-v2/README.md)。
