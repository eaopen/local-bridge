# Local Bridge 通用任务中心 P1：只读任务聚合

2026-10-29。**已在本机 CLI 提供只读聚合，并新增独立的可选 Tauri Desktop 开发预览视图；Web API、MCP Tool、任务数据库及文件动作仍未提供。** 桌面只经本地 Rust Commands 调用相同的 `task_center::list`。

## 使用方式

```sh
local-bridge tasks list
local-bridge tasks list --cloudfile
```

- `tasks list`：即时从本机已有 Codex/Pi Agent V2 worker、WorkBuddy worker 和 Inbox 读取状态。**不**启动 AI、访问模型、不读取提示词或模型回复、不修改任务/授权、不检查远端登录。
- `tasks list --cloudfile`：除上述来源外，**用户显式**选择再向已安装、已注册为 `cloudfile` 的 MCP Provider 调用一次 `cloudfile_task_summaries`，展示其脱敏的现有 Agent2 文件任务。未注册或 Agent2 版本不支持该工具时，来源状态显示 `not_configured_or_unavailable`、`complete=false`；**不会假装文件任务为空或已结束**。
- CLI 命令与可选 Tauri Desktop 都只在当前 OS 用户本地使用。网页、模型和一般 MCP 调用**没有**获得 `tasks.list` 权限，不能通过网页自动列出其他 Origin 的 AI 任务或 CloudFile 本地文件任务。
- 输出统一的 `schema_version=1`、`read_only=true`、`local_cli_only=true`、`complete`、`sources` 与 `items`；每条映射原任务 `task_id`、`owner_module`、`kind`、`provider`、`status`、`created_at_unix_ms`（如果本模块有时间戳）、`available_actions=[]`。每个来源失败单独标记，不丢弃其他来源。
- 不返回 AI prompt、对话内容、会话来源 Origin、模型访问令牌；Inbox 只返回已存储的标题（最多 80 字符）和 ID，不回显选中文本；CloudFile 只返回文件模式与原 Agent2 Task ID、冲突/待确认状态，**不返回文件名、云端路径、本地路径、服务器、库 ID、票据或设备私钥**。
- Inbox 的 `needs_action` 只表示待用户处理的已存储内容，**不等于 AI/CloudFile 执行失败**。Agent 的 `message.final` 也**不是**任务完成信号；必须由 owner 明确产生 `task.completed`。CloudFile 本地保存/Checkout 不代表已成功云端发布，原 Journal 中未确认提交映射 `unknown`。

## 数据拥有者与性能

| 来源 | 事实源 | 读者 |
| --- | --- | --- |
| Codex / Pi | Bridge 原有 `agent-v2-tasks` metadata + cursor event log | 只读 Owner 投影，最多 40 个最近任务 |
| WorkBuddy | Bridge 原有 `workbuddy-web-v2` metadata + event log | 只读 Owner 投影，最多 40 个最近任务 |
| Inbox | Bridge 原有 `inbox` | 已有 `inbox list`，最多 100 条，本次不复制内容 |
| CloudFile（显式启用） | Agent2 原 `storage::root()`、Journal/Guard | 由 Agent2 Provider P1 提供最多 80 条本地脱敏状态，Bridge 只做严格字段白名单映射 |

没有新的 `tasks.json` 或 Task DB。AI worker、Inbox 及 Agent2 Tauri 继续原样工作，Bridge 只提供合并视图，不在其中计算“应该自动上传/重试/授权”的决策。Agent2 Provider 目前仍在 [独立 Draft PR #26](https://github.com/eaopen/cloudfile-local-agent2/pull/26)，**未合并 Agent2 dev**；所以仅凭 Bridge dev 的新 CLI 不代表任意已安装的 Agent2 都具备文件摘要能力。

## 兼容与验收

- 新 Chrome 与原 CloudFile Chrome 保持各自 ID/Host/发行；**一份 CloudFile 文件目录与一个 Agent2 Writer**，Bridge 不从其他路径读取旧任务。
- **CLI 聚合模块本身**不新增第三方依赖、常驻 HTTP、跨程序文件执行或 File API；独立可选 Tauri Desktop（PR #36）属于单独编译单元，不改变 CLI 的 WebView2 依赖。
- 单元验证 Agent 状态投影、Inbox 纯只读映射、CloudFile 响应敏感字段过滤/错误拒绝。跨进程测试使用隔离用户目录，验证真实 Bridge CLI 可读取既有 Inbox 记录。
- 只有以后明确的用户授权、来源隔离和任务聚合 Web UI 需求才能讨论受控 Web 查询入口；P1 **不自动开放**。
