# WorkBuddy Web V2：异步任务与多次消息返回（第一阶段）

此实现使用独立的 Local Bridge Native Host + 新通用 Chrome 插件；不迁移 CloudFile V1，也不修改 etech-bpm。

## 1. 为什么不是一次 sendNativeMessage 等待完成

Chrome Native Messaging 的一次性调用不能可靠承载多次 WorkBuddy 回复，更不能将 HTTP/Open API 的 accepted 当 completed。采用短期本机任务记录，保持 Native Host 快速返回：

1. `agent.start`（Chrome V2）→ `web.v2.agent.start`（Native V2），提交唯一 `client_key` 和界限内的文本，回复 `status=accepted, task_id`。
2. 独立 WorkBuddy worker 通过用户既有 WorkBuddy Desktop 的 WBIPC + bundled CLI/ACP 运行，所有本机工具与 MCP 禁用；收到的 `session/update` 转为有序事件。
3. `agent.poll`（浏览器 V2）→ `web.v2.agent.poll`，提交 `task_id + cursor`，返回最多 40 个新增事件、`next_seq`、`has_more`、`status` 与 `terminal`；跨窗口/刷新可续读。
4. 单次 WorkBuddy assistant 回复可包含多段 `message.delta`；每条逻辑回复以 `message.final` 标记。工具信息是 `tool.update`，最终只在 ACP `session/prompt` 的 `stopReason=end_turn` 确认并且有真实助手输出后产生 `task.completed`。文本/工具事件都**不是**整个任务完成。
5. `task.failed` 或超过运行期限无终判则显示 `unknown`；**不自动重试、不切换 Codex/Pi**。网络回执丢失时使用同一个 `client_key`，服务端寻找原 task，不重新提交。

### 示例事件序列

```json
{"seq":1,"type":"task.started"}
{"seq":2,"type":"message.delta","message_id":"msg-1","text":"第一部分"}
{"seq":3,"type":"message.final","message_id":"msg-1"}
{"seq":4,"type":"message.delta","message_id":"msg-2","text":"补充说明"}
{"seq":5,"type":"message.final","message_id":"msg-2"}
{"seq":6,"type":"task.completed","stop_reason":"end_turn"}
```

Web 按 seq 去重、按 message_id 合并分片；刷新后从 cursor 0 重读原 task，再恢复显示。不自动合并进旧元枢聊天记录，用户可以主动选择与原元枢交接的内容。

## 2. 首期权限与安装

受信 Etech Web Origin 需要 **两级授权**：在 Chrome 插件 Popup 中对该站点单独启用 text-only WorkBuddy；在同一台电脑当前 OS 用户下显式运行：

```sh
local-bridge web-v2 grant-workbuddy https://your-etech-web.example
local-bridge web-v2 workbuddy-capability https://your-etech-web.example
# 停用
local-bridge web-v2 revoke-workbuddy https://your-etech-web.example
```

Chrome Service Worker 必须以真实 sender.origin 验证来源和 top-frame；Native Host 按 origin 二次核验本机授权。任何网站/Profile/LLM 文本都无法自行授权，也不能传命令、工具列表、工作目录或 WorkBuddy 凭据。用户点击元枢新面板才发起任务，默认仅文本，无法读写文件、执行 Shell/MCP。**由 WorkBuddy 本人既有登录与权限处理模型费用和实际响应。**

当前只对已验证过 ACP 实验的 **macOS WorkBuddy Desktop + Node 24+** 开放，且仍需安装软件的真实用户机验证。无 WorkBuddy 时显示不可用，原 Etech Web 和 Chrome v1 Popup 不受影响。Node 仅为可选 WorkBuddy Adapter 使用，不成为 Local Bridge 核心依赖。

## 3. 本地事件记录与生命周期

每个任务有独立 OS 用户目录（Unix 权限 0700）以及事件 JSONL（0600），不保存浏览器 Cookie、EAP Token 或模型 API Key。最多 250 条事件、512 KiB、30 分钟有效；旧任务在后续使用中顺手清理，**不是精确到点的定时删除保证**。Worker 最长约 170 秒，Chrome 通过短轮询读返回；单页初版只启动一个任务，结果 unknown 时保留原记录，用户必须手动决定是否新建任务。

本机 WorkBuddy 5.9.3 的 WBIPC / bundled CLI ACP 验证来自之前实验；内部协议尚无稳定兼容承诺。本版本**未**验证 Windows、真实已登录 WorkBuddy 设备、桌面 GUI 已有会话/专家继承、ACP 工具审批后的复杂任务与真实企业 Web 端到端。不能凭 Chrome Node 测试或 Rust CI 报告已联调成功。

## 4. 阶段边界

- P0：Web 元枢一个短文本任务、支持多段回复/状态/断线恢复；Mac WorkBuddy 用户机实测之后收口。
- P1：在实际 WorkBuddy 已完成时考虑可交互授权的工具、更多事件类型；再支持其它 Agent Provider。
- P2：本地企业定制套壳元枢；CloudFile 独立评估稳定后迁移。

关联：[Local Bridge #6](https://github.com/eaopen/local-bridge/issues/6)、[#22](https://github.com/eaopen/local-bridge/issues/22)、[EAP #44](https://github.com/jcetech/etech-eap/issues/44)。
