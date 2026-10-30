# 微信片段接入验证（2026-10-30）

## 已实现

桌面“帮我聊”新增可折叠的微信片段面板：用户检查连接、指定会话显示名、选择已有索引或可用的实时来源、预览并勾选最多 10 条片段，再放入本轮输入。只有点击生成建议才调用所选 Agent。支持清除本轮；不自动读取、监听或发送。

Bridge 调用独立 Provider 的六工具契约，不增加微信 SDK 依赖。新版读取要求 read_contract_version=1，严格区分 local_index / desktop_ui；拒绝会话或来源不匹配，标记部分范围和非稳定显示名。旧 Provider 提示升级，缺失 Provider 仍可粘贴使用。

## 验证

- Rust workspace：82 项通过；严格 clippy 通过。桌面 Node：15 项通过。
- 真实 Provider stdio + 隔离临时虚构索引 + 真实 WorkBuddy（glm-5.3-flash / low）：13.446 秒完成，生成三档候选。示例入口 crates/local-bridge/examples/chat_source_validation.rs 仅用于人工构造的隔离验证环境。
- 浏览器模拟 Tauri：展开不读取，检查只请求状态，读取带明确会话与条数，取消勾选的内容不进入 Agent；选入不触发推理，生成携带来源，清除移除预览。
- 桌面开发环境已显式注册本地 Provider；MCP 响应且索引读取可用，Mac 返回 macos_login_safety_hold，实时选项禁用。没有读私人聊天或操作微信。

## 未验收

真实微信读取及发送均未验收。当前无在线 Windows 交互设备；Mac 4.1.15 既往缺失 AX 控件并发生重登录，继续遵守 Provider 的登录保护约束。候选源码评审位于 wechat-provider/docs/MAC_CANDIDATES_2026-10-30.md；优先验证指定窗口被动截图 + 本机 OCR，不直接引入自动回复/发送工程。

构建使用 cargo +1.90.0（系统默认 stable 当前仍是 1.88，低于项目要求）。本地已注册不代表新安装自动注册；Provider 仍是可选依赖。
