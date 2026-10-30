# Chat Copilot P0-A 本地验收（2026-10-30）

范围：`feat/chat-copilot-p0`，基于 `dev` 的 P0 方案。只实现手动文本入口；未操作微信、微信账号、Provider 或真实聊天。

## 已实现

- 原 Desktop 增加「帮我聊」；三个意图、少量表达偏好、事实/推测/未知、2～3 个可编辑草稿、复制、不回复、五种本轮反馈和清除。
- `chat_copilot` 构造平台无关输入，复用 `agent_options` 的偏好、选择算法、`RunAgentRequest` 验证及 Codex/Pi/WorkBuddy 适配器；生成前显示固定的 Agent/model/think，失败不换 Agent。
- 原 Web `runAgent/readAgent` 使用磁盘任务记录，不能直接用于私聊。桌面提供相同 run/read 生命周期的 **单槽内存模式**，不创建第二队列、任务目录或 Web Origin 授权。相同 client key 返回同一调用；忙时拒绝另一个调用；读取失败只查询，不重新提交。
- WorkBuddy 原 worker 增加 stdout 输出选项；原 Web 文件输出保持默认。临时目录仅承载 worker 代码和隔离运行环境，不保存 Bridge prompt/events。Pi 禁用工具、MCP、扩展、上下文文件和会话保存。
- Codex 沿用 CLI 原账号和临时会话，禁用 shell/unified exec、MCP、插件、应用、浏览器、hook、记忆等入口；保留 read-only 沙箱作为文件写边界。包含工具活动的结果不作为聊天成功返回。**不把 read-only 或 feature 配置声称为所有 CLI 版本均不存在任何内置工具**。
- Codex 聊天调用使用 `--ignore-user-config` / `--ignore-rules` / `--strict-config`：原认证仍由 Codex 管理，模型/思考参数由 Bridge 偏好显式传入；自定义 Codex provider/profile 和 Pi 扩展模型不在此切片兼容范围。旧版本不支持参数会显示失败，不静默退回宽权限执行。
- Bridge 不将内容写到日志、任务中心或 localStorage；页面刷新/退出不恢复会话；成功读取后清掉后端结果，只保留可编辑页面文本。远端 Agent 服务的数据处理、用户主动复制的系统剪贴板不由 Bridge 清除。

## 自动化与交互证据

- Rust 1.90：核心库 62 项单测通过（含合成输入隔离、长度约束、同 key 去重、忙时拒绝、读取清除、工具回执拒绝）。
- Tauri：macOS `cargo check` 和桌面单测通过。
- Node：桌面 13 项单测通过；新增解析、畸形/超长输出、编辑后复制、剪贴板失败、失败/超时不重发检查。app/chat/worker 语法检查通过。
- Playwright **模拟 Tauri IPC**：生成→编辑→复制，反馈用于下一次生成，不回复，清除，非 JSON 原始草稿回退，无 Agent 禁用生成。浅色和 760 px 宽深色布局已查看，无横向溢出。此项不是原生窗口或系统剪贴板实机验收。

## 真实 Agent（与模拟 UI 分开）

使用 `cargo +1.90.0 run -p local-bridge --example chat_copilot_validation -- codex`，四条文本均为代码中的虚构样本，沿用原有 Codex 登录，无新增 API 凭据。最终受限参数下的执行结果：

| 样本 | 模式 | 结果 | 耗时 |
| --- | --- | --- | --- |
| 朋友出差疲惫 | 回应 | completed，结构化建议 | 11.7 s |
| 工作礼貌拒绝 | 回应 | completed，结构化建议 | 14.7 s |
| 道歉且尊重冷静空间 | 理清自己 | completed，结构化建议 | 16.7 s |
| 两天未回复、原因未知 | 理解 | completed，事实/推测分离 | 13.5 s |

开发中发现 Codex 将 CLI 配置告警也作为 `item.error` 输出；已区分诊断与工具回执，并加回归测试。不能把一条配置告警误当成工具执行，也不能把未完成 turn 当成功。

## 尚未验收

- 原生 Tauri 窗口到真实 Agent 的完整人工点击，以及真实系统剪贴板；Windows 实机。
- 本次聊天路径的 WorkBuddy、Pi 真实模型执行；它们复用已有适配器，但未借用历史验证冒充本轮通过。
- 不保证模型建议永远符合事实；用户仍需核对、修改。四例验证不是质量评估全集。
- P0-B 的发现/连接/读取/发送、长期记忆、快捷入口、群摘要、Soul 均未包含。

复核命令：

```sh
cargo +1.90.0 fmt --all --check
cargo +1.90.0 test -p local-bridge --lib
cargo +1.90.0 check --manifest-path crates/local-bridge-desktop/Cargo.toml
cargo +1.90.0 test --manifest-path crates/local-bridge-desktop/Cargo.toml
node --test crates/local-bridge-desktop/ui/*.test.mjs
```

配置依据：[Codex 官方配置参考](https://learn.chatgpt.com/docs/config-file/config-reference) 及本机 `codex exec --help`。工具配置随 CLI 版本变化；实机记录仅适用于本次安装版本，不等于全版本认证。

## 原生桌面追加验收（同日，本地更新）

- 本地旧进程已替换为本分支构建，开发入口 `/tmp/Local Bridge Desktop Dev.app` 已启动，出现「帮我聊」和生态接入入口；原开发二进制已备份为 `.before-chat-p0`。
- 通过 macOS 原生窗口点击，使用同一虚构朋友对话完成真实 Codex 生成，页面显示事实/推测/未知与三条候选；编辑第一条、点击复制、在第二条用系统粘贴，核对内容完全一致。
- 原生点击「自然」反馈、「这次不回复」和「清除本轮」均显示相应回执；清除后输入与结果为空。当前本地默认 Agent 已保存为 Codex（model/think 均 auto），应用保持打开。
- WorkBuddy 固定短任务检查成功，但本次真实聊天调用失败；页面如实显示未完成，未自动切换。具体失败原因未定位，不视为 WorkBuddy 聊天验收通过。Pi 固定短任务未通过。
- PR 初次核心 CI 因 `items_after_test_module` 失败；调整两个文件的函数位置后，本地严格 Clippy、62 项核心单测通过，修正提交 `d2b295f` 的三系统核心 CI 均成功。
- 此次补齐 macOS 原生窗口→真实 Codex→编辑→系统剪贴板闭环；Windows 实机和 WorkBuddy/Pi 聊天可用性仍待验收。微信未操作。

## WorkBuddy 失败排查与局部修复（历史阶段，后续结果见末节）

- 已修复可离线复现的 worker 缺陷：原先每个 token 小片段占一个事件，251 个片段就超限；现在合并至约 1024 字符，保持快照/消息边界与 Unicode 完整。新增四个 Node 回归，并接入三系统 CI。
- 保留原事件与输出字节上限、不增加自动重试。私聊 worker 的 `task.failed` 现在可返回白名单安全提示，区分上游未接受、超时、输出超限；不回显原始错误或私聊。
- 实际完整聊天失败另有上游原因：捕获到原账号 Broker 的 `E_UPSTREAM`，CLI 随后返回 502 / `stopReason=refusal`，没有聊天文本。安装的 WorkBuddy `wb.request/http.fetch` 实现缓冲完整响应，并固定 50 秒 abort；完整样本约 51.7 秒失败，与该限制相符。没有读取或导出账号凭据。
- 一次较短的虚构结构化请求真实完成，但完整 Chat Copilot 样本仍未通过。低 effort、临时关闭额外思考及压缩提示/输出的试验仍约 52 秒失败，已撤回这些无效参数与提示改动；不能把短请求成功声称为聊天已修复。
- 当前仍需 WorkBuddy 提供适合长响应的已授权接口，或在原客户端解决本次模型/网络延迟。Bridge 未改动 WorkBuddy 安装、代理超时或登录，不绕过其通道限制。
- 严格 Clippy、63 项 Rust 单测、四项流式回归与桌面构建通过；本地默认 Agent 恢复 Codex。WorkBuddy 完整聊天验收仍未通过。


## WorkBuddy 模型参数修复与真实验收

- 原 `auto / auto` 的完整请求再次约 52 秒以 `E_UPSTREAM / 502` 结束。仅缩短系统提示词没有解决；账号实时目录显示 `auto` 配置为 high 推理。原通道仍是固定 50 秒、缓冲完整响应，不宣称已解除该限制。
- 对照实验保留原 worker 提示词和授权链，仅用官方 CLI `--model glm-5.3-flash --effort low`，完整聊天正常完成；请求元数据确认实际传递了这两个参数。最终修复不包含实验性的提示词覆盖、禁用 reminder 或上游改写。
- Desktop 偏好、统一 Agent API、本地聊天、Web worker 和短任务检查现在传递显式模型及思考强度。旧调用仍保持 auto 默认。worker 在发送聊天前用当前账号目录校验主 Agent 可用模型及该模型声明的思考强度；未知模型、不支持的强度明确失败，不静默替换、不自动重试、不改变账号凭据。
- 通过 `chat_copilot_validation -- workbuddy glm-5.3-flash low` 完成四个虚构场景：friend 19.531 秒、work 19.245 秒、repair 18.226 秒、silence 8.603 秒。前三个返回可解析的 2–3 条草稿；silence 首次完成调用但返回空候选列表，前端如实降级原始草稿。随后明确三个 mode 均应返回 2–3 条可选草稿，单独复测 silence 13.909 秒完成并通过前端 `parseDraft`，返回三条草稿。提示词约束不保证模型永远遵循格式，原始草稿回退仍保留。
- macOS 原生窗口已保存 WorkBuddy / glm-5.3-flash / low；真实点击生成，显示完成、事实/推测/未知与三档可编辑草稿。最终二进制已更新，重启后显示同样的模型及强度。未操作微信。
- 检查：79 项 workspace Rust 测试、35 项 Web SDK、桌面与 WorkBuddy Node 测试、严格 Clippy、Desktop 构建均通过。此记录不代表所有账号/模型/输入在 50 秒内均可完成，也不代表 Windows/macOS 微信实机验收。

- 最终构建的原生 understand 请求也确认完成，但模型在单个 JSON 代码块之后附加解释。补齐解析兼容：仅提取一个有界 JSON 块，尾随说明作为纯文本保留显示；多个区块、损坏 JSON、过长说明仍回退原文。新增针对这一真实返回形态的回归测试。

## 紧凑布局与聊天软件展示预留

- 960px 起上下文与回复双栏，窄窗口改为上下排列；输入片段缩至四行，草稿优先显示，事实/推测/未知和隐私说明按需展开。复制按钮紧邻每条草稿，反馈合为一行。
- 微信预览区默认收起，预留会话、消息范围、读取状态与所选消息展示。明确标记尚未接入、未读取，不显示虚构联系人，不调用 Provider、不启动微信、不提供发送操作。
- Playwright 使用模拟 IPC 和虚构草稿验证 1280px、1000px 双栏及 760px 深色窄屏无横向溢出；编辑后复制内容一致，反馈、展开依据、清除和空态正常。点击微信预览未产生 Provider 请求。这是 UI 验证，不计作新的真实模型验收。
- 14 项桌面 Node 测试、Desktop 构建和 diff 检查通过；本机原生窗口已更新并核对微信预览展开、Agent/model/think 展示。聊天调用与权限逻辑未变。
