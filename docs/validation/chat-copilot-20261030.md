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
