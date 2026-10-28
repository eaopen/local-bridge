# BridgeTask V1 — 跨模块任务中心合同

状态：**本机只读 CLI 聚合已在独立功能分支实现；Web/跨来源任务 UI、订阅与动作转发仍未实现**。文件、企业应用、个人 AI 与通讯任务可以进入**第二套 Local Bridge 产品**自己的任务中心，不创建第二份文件编辑/AI 执行账本。**Bridge 通过获准的 Provider 只读展示同一份 Agent2 文件任务，绝不新建第二套文件数据目录或状态账本**；见[两套独立产品边界](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

**已实现的本机只读使用说明**：[任务聚合 P1](TASK_CENTER_P1.md)。CLI 汇总既有 AI/Inbox，并在本机明确指定 `--cloudfile` 时访问 Agent2 Provider 的脱敏任务摘要；不开放 Web/MCP 通用任务读取权限。CloudFile Agent2 dev 未合并 Provider P1 前，该来源如实标为不可用。

## 1. 所有权

任务由`owner_module`产生并对专业状态负责。Local Bridge 只负责聚合、展示、用户发起的操作分派与本机通知。

- `cloudfile`：现有 Agent2 File ID/Branch、Journal、Intent、Guard、Checkout/Commit/Checkin 是**唯一文件发布事实源**；Provider 读取和后续转发已授权操作，Bridge 不建第二个 Writer。
- `enterprise`：企业 API/桌面程序的完成与失败由相应业务模块判定。
- `ai` / `pi`：会话、推理、工具事件由企业 AI 服务或 Pi 管理；Bridge 只显示明确的 run/task 状态。

任务中心可以汇总正在执行的 AI run；**不要**把每条聊天消息、每次工具 token 或浏览器导航都永久记录成 BridgeTask。

## 2. 最小结构

```json
{
  "schema_version": 1,
  "profile_id": "sample-app-prod",
  "task_id": "owner-assigned-id",
  "owner_module": "cloudfile",
  "kind": "file",
  "capability": "cloudfile.file.exclusive",
  "title": "Edit drawing",
  "context": {"object_id": "object-001"},
  "status": "needs_action",
  "progress": null,
  "available_actions": ["open", "commit", "checkin"]
}
```

- 身份键为 (`profile_id`, `owner_module`, `task_id`)。不要仅以文件名或原始云端路径匹配任务。
- `profile_id` 关联任务来源与用户上下文，**不是默认隔离墙**；个人聊天助手可调用已有授权的 CloudFile/企业工具，具体业务提交仍由各自拥有模块终判。
- `kind` 为视图分类（如 `file`、`enterprise`、`ai`、`local_tool`），不是执行权限。
- `context` 是应用拥有的提示/引用信息；通用任务 UI 只能展示已允许的字段，不解释成命令、账号凭据、工作目录。
- `available_actions` 由拥有该任务的模块实时计算，不由网页或 AI 自行拼装。
- `progress` 允许 null，未知或无法量化时不捏造百分比。

## 3. 通用状态

| `status` | 统一语义 |
|---|---|
| `queued` | 已接收、等待执行 |
| `running` | 模块正在执行，结果尚未确认 |
| `needs_action` | 用户必须确认、提交、解决冲突或重新授权 |
| `completed` | 拥有模块确认其事实链已完成 |
| `failed` | 已确认失败，但保留可恢复数据 |
| `cancelled` | 拥有模块确认终止 |
| `unknown` | 结果未知；不得假定成功或失败，不得因展示层超时重复执行 |

CloudFile 的“本地已保存”**不等于**`completed`（云端已发布）；独占编辑的已 Checkout 也不等于文件已 Commit。CloudFile 仍保留自己的详细状态，映射到 Bridge 状态时需附带可读说明及必要操作。

### 同机个人与企业混合任务（目标）

当 Agent2 Provider 具备可靠的只读任务摘要后，同机 Bridge 可显示**现有 Agent2 文件任务**与个人通讯/AI 任务；Agent2 Tauri 仍同时显示相同文件状态。BridgeTask 只建立跨模块的视图引用，不复制企业文件、历史聊天或 CloudFile Journal，也不因为多出一个 Chrome 入口而生成第二个文件任务。

## 4. 任务接口（规划）

- `list_tasks(profile_id?)`：按当前可信应用和权限展示任务摘要；
- `get_task(profile_id, owner_module, task_id)`：获取详情，敏感路径和票据不返回通用 UI / MCP；
- `task_events`：事件订阅和重连后快照，仅状态变化，不创建业务提交；
- `request_action(task_ref, action_id, args)`：转发给 owner 模块再次鉴权。发送成功**不等于**动作完成。

Bridge 只维护低成本索引/展示缓存；工作副本、快照、Intent、服务端原生版本和 Pi 会话各归原模块。不存在任务“迁入 Bridge 新表”的默认步骤。

## 5. 失败恢复与迁移

- Agent2 原工作副本/Journal/Guard 仍是唯一文件状态源；Bridge 可以经用户授权的 Provider 接收**同一个 Agent2 文件任务的脱敏摘要**，但不直接读取原本机目录、不保存/改写 Journal。**先读后写**，写入转发独立验收。
- 桥接不可用时，CloudFile 现有文件任务入口仍可独立工作；UI 应说明功能降级，不改写原任务状态。
- 不允许 Bridge、Pi、MCP 因网络错误自动再发一次新的文件提交。结果未知时查询原操作；需人工重试也必须遵守 CloudFile 原有幂等合同。
- 结束、退出、升级、卸载均不能删除未确认工作副本或自动释放仍有效的 Checkout。

V1 从一个**原 Agent2 文件任务的脱敏摘要**开始验收；Bridge 不导入旧数据副本，也无需先建通用任务数据库。
