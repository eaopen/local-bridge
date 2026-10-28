# Web V2 Agent：统一状态、选择、执行与读取（2026-10-28）

> **实现状态：代码已接线，尚未做真实登录、模型、Chrome、Windows/macOS 端到端验收。** 以下是当前 `dev` 候选代码合同，不把源码存在或 CI 通过当作实际 Agent 可用。

## 公共合同

三个 Provider 统一通过 `runAgent/readAgent`，请求仍是 `api_version=2`：

- `agent`: `auto | workbuddy | codex | pi`（不指定时为 auto）。
- `agent_mode`: `prefer | require`；`require` 只选该 Provider，`prefer` 可在**提交前**选择其他符合要求的 Provider。
- `model` / `think_mode`: 默认 `auto`，具体支持由所选 Provider 决定。不支持时在提交前拒绝，不静默忽略。最终 `applied_model/applied_think_mode` 在当前代码中为 null，表示**未核验实际生效值**。
- `client_key` 必须在 HTTP 失败转 Native、刷新重试中保持相同；所有三个 Agent 在新建任务前共同查询已有任务，防止换通道时自动重跑。
- `POST /v2/agents/tasks` → `accepted/task_id/selected_agent`。仅 accepted 表示本地记录/worker 已安排，不表示成功。
- `GET /v2/agents/tasks/{task_id}/events?cursor=N` 或 `/stream?cursor=N` → `seq` 事件、`next_seq`、`status`、`terminal`。Native 备用通道仍用 `agent.run` / `agent.read`；无厂商专用 Web API。
- `message.final` 结束一条回答，只有 `task.completed` 或其他终态才能结束任务；`failed/unknown` **绝不跨 Provider 自动重试**。

### 统一 Agent 状态

`GET /v2/capabilities`、Native `web.v2.capabilities` / Chrome `local.capabilities`，以及 CLI `web-v2 capabilities <origin>` 均返回 `agents` 数组。每个条目包含：

| 字段 | 含义 |
| --- | --- |
| `agent` | `workbuddy / codex / pi` |
| `installed` | 根据当前 OS 的可执行文件/现有安装路径发现，不代表登录成功 |
| `local_grant` | 当前 OS 用户是否对精确 Web Origin 授权该 Agent |
| `run_supported` | Bridge 当前代码是否支持对应平台的文本任务 |
| `eligible` | 可**尝试提交**（installed + grant + run_supported），非 ready |
| `authenticated`、`api_ready`、`ready` | 当前全部为 null，表示**未核实**；不从 installed 推导 |
| `state` | `not_installed / unsupported_platform / not_granted / unverified` |
| `task_mode` | WorkBuddy/Pi 为 `text_only_no_tools`；Codex 为 `read_only_sandbox`，允许 Codex 在只读沙箱中使用其内置能力 |
| `supports_model_override` / `supported_think_modes` | 代码支持的参数范围，非模型实际能力证明 |

此处只做轻量本地安装发现与权限读取，**不主动登录、消耗额度或调用模型**。默认 `auto` 的尝试顺序为 WorkBuddy → Codex → Pi（仅在已授权、已安装、参数支持时进入候选）；用户显式 `prefer` 会优先于默认顺序。未来接入可信 ApplicationProfile 策略后可以替换默认顺序，当前不要把尚未激活的远程 Profile 当作权限。

## 三种现有执行方式

| Agent | Web V2 提交后执行方式 | 结果与完成依据 | 参数限制 |
| --- | --- | --- | --- |
| WorkBuddy | 原 macOS Desktop 的 WBIPC + bundled CLI/ACP，现有独立 Node Worker | ACP 的 `end_turn` 与实际助手回复；可有多条 message | 仅 `model=auto`、`think_mode=auto`；当前 macOS |
| Codex | 已有 `codex exec --ephemeral --sandbox read-only --json`，独立轻量任务 Worker | Codex JSONL `turn.completed` + 有效 assistant message | `--model`；`-c model_reasoning_effort=` 支持 low/medium/high/xhigh |
| Pi | 已有 `pi --print --no-session --no-tools` 的单次文本 CLI | 正常退出且有非空文本（尚待实机确认实际状态语义） | 可指定 `--model`、`--thinking`（off/low/medium/high/xhigh）；不支持 max |

**注意：Codex `read-only` 只限制写入，不等同于禁止读取本机文件。** 只对用户明确授权的可信 Web Origin 启用；浏览器不能直接传 Shell 或工作目录，但 Codex 的内部只读工具仍可能读取本机文件。若实际业务要求严格无本地文件读取，应只授权 Pi/WorkBuddy 文本模式，待 Codex 有可信无工具执行策略后再开放。

Codex/Pi 共用轻量持久化 task.json + seq 事件，启动的是 Bridge 当前已安装二进制的独立 worker（Native Host 和 HTTP 都能创建）；它不继承网页进程寿命。Bridge 只保存本轮引用、短期请求/事件；不复制三家账号、会话/模型路由或文件引擎。后续可依据真实平台能力逐项增强，**不是已完成三个正式生产 Adapter 的宣称**。

Pi 保留用户已经配置的模型 Provider/扩展供给，但用 `--no-tools` 禁用模型工具、MCP 调用、文件写入；这是用户已安装本地扩展自身的信任边界，不给网页开放任意命令、扩展路径或参数。企业 EAP 的短期 Pi 票据/服务 API 仍走现有受信专用路径，不能直接从未经验证的网页参数赋予 Bridge。

## 授权、回退与兼容

仅 OS 用户可以明确批准某个站点 Agent，Chrome 插件还需单独批准精确站点：

```sh
local-bridge web-v2 grant-agent https://your-application.example codex
local-bridge web-v2 grant-agent https://your-application.example workbuddy
local-bridge web-v2 grant-agent https://your-application.example pi
local-bridge web-v2 capabilities https://your-application.example
local-bridge web-v2 revoke-agent https://your-application.example pi
```

旧的 `grant-workbuddy/revoke-workbuddy` 仍指向 WorkBuddy 原有授权文件，已授权站点不丢权限。新增 Codex/Pi 授权独立存于当前 OS 用户的数据目录（Unix 私有文件）。从浏览器打开的任何 Profile/消息都**不能**自行授权 Agent。

Chrome 负责可信 Origin/用户同意/短期会话；Bridge 默认本机 `http://127.0.0.1:48293` + SSE，失败时 Chrome Native 在同一个 `client_key`/task ID 上续接。无扩展、Bridge 未安装、站点未授权时企业原网页照常运行；CloudFile Agent2、文件三模式、EAP 原元枢和 V1 Host 均不改。

## 完成与剩余验证

**代码级完成**：统一状态 JSON、参数规则、权限、分配、双通道 idempotency、三条可运行的本地 Worker 路径、统一读取/终判；增加纯单元/合同测试。**未完成**：三个 Agent 的真实登录/模型额度/流式结果核验、Codex/Pi 的真机默认模型兼容、Pi 扩展及推理档位实际生效、Windows/macOS 的浏览器多通道端到端、企业 SSO/企业 Pi API 端到端。

最小后续验收应分别在实际装有 Agent 的普通用户机进行：`not_installed` → `not_granted` → 申请授权 → 短任务 `accepted` → 确认 `completed` 或真实失败 → HTTP/SSE 中断转 Native 续读、不重复执行。不要把构建通过、安装发现、`accepted` 或单条 `message.final` 记录为 Agent 已完成。
