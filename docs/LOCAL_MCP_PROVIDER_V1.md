# 本机 MCP Provider V1：微信 Provider 实际接入

**状态：** Local Bridge 提供可选的、用户主动注册的本机 MCP stdio Provider 调用；WeChat Provider 自己提供六个工具、SQLite 索引、图形界面自动化与消息发送账本。两者可独立安装和使用。

## 使用步骤（用户本机操作）

1. 根据 [wechat-provider](https://github.com/eaopen/wechat-provider) 的 README 安装 WeChat Provider，并安装对应系统的可选依赖；需要真实登录微信和图形界面权限，先自行检查上游 Provider 的实际状态。
2. 找到**已安装**的 `wechat-provider` 可执行程序的绝对路径。使用 Local Bridge 显式注册，命令中的路径必须替换成自己实际安装的位置：

```sh
local-bridge provider add wechat /absolute/path/to/wechat-provider serve
local-bridge provider list
local-bridge provider tools wechat
local-bridge provider call wechat wechat_status '{}'
local-bridge provider call wechat wechat_list_chats '{"limit":5}'
local-bridge provider call wechat wechat_get_messages '{"chat_id":"文件传输助手","limit":10}'
local-bridge provider call wechat wechat_search '{"query":"待办","limit":10}'
```

可从标准输入传入 JSON 参数，避免复杂 shell 引号：`printf '%s' '{"query":"待办"}' | local-bridge provider call wechat wechat_search -`。为了避免检索整个通讯软件，必须选择具体聊天并使用合理的上限；发送时也应使用明确的测试对象。

**发送为显式本地 CLI 操作，不被网页或 AI 自动调用：**

```sh
local-bridge provider call wechat wechat_send_message '{"chat_id":"文件传输助手","text":"测试消息","idempotency_key":"test-20261028-001"}'
local-bridge provider call wechat wechat_get_send_status '{"task_id":"<the-original-task-id>"}'
```

发送前核对聊天对象及账号。**当前 Provider 的 chat_id 是显示名称而非稳定身份**；`submitted` 仅意味着 UI 自动化程序已返回，不等于实际送达。发生 `uncertain` 时不要自动重发。请只在个人已登录的受控测试聊天中做真实发送验收。

卸载本机注册：`local-bridge provider remove wechat`。WeChat Provider 不依赖 Local Bridge；取消本机注册不会删除 Provider 的 SQLite 或本身配置。

## 通用架构，不做微信特殊分支

- `provider add <id> <absolute-executable> [fixed-args...]` 是**本机用户主动批准**已安装程序的操作。只保存固定程序/参数的本机记录（按 OS 用户），不下载可执行文件、不存账号密码、不执行网页/消息中的命令。
- Bridge 作为 MCP Client 先完成 `initialize`、`notifications/initialized`、`tools/list`，随后经本机 CLI `tools/call` 只调用真实宣布的工具。不会把未验证的工具列表写成“已登录微信”。
- Bridge 的 MCP Server 和 Chrome Native Host 仍各自保持现有只读/手动上下文交接能力。**注册的 Provider 工具不会自动暴露给 Chrome 页面、已连接模型或任意第三方进程。**
- 真实微信内容仅作为此次用户显式查询的响应返回终端；Bridge 不复制原聊天数据库、不自建消息索引或发送账本；结果属于 WeChat Provider。
- 用户在同一电脑使用企业 CloudFile 和个人微信时无需切换 Profile，默认共享本机已授权能力。但企业文件权限由原服务判定；无需额外建设企业 DLP。
- 本功能目前**没有自动消息订阅**：WeChat Provider 只公布 `wechat_list_chats/get_messages/search/send/get_send_status/status`，尚无 `messages.subscribe`。将来如 Provider 增加订阅，优先接它实际实现的接口。

## 验收与剩余事项

- Rust CI 应通过 Linux、macOS 和 Windows 的 fmt、Clippy、unit/build，独立检验本机配置与 JSON-RPC 结果约束。
- **已验证：**独立 Python WeChat Provider 与 Rust Bridge 的跨进程 MCP handshake、六工具声明、`wechat_status`、空数据库 `wechat_search` 和未知工具拒绝；见 [实际 CI 记录](https://github.com/eaopen/local-bridge/actions/runs/40563141307) 与 [测试方法](WECHAT_PROVIDER_MCP_SMOKE.md)。**仍未验证：**Windows/macOS 已登录微信账号的真实 GUI 会话读取、发送对象身份与送达情况。
- 下一步再做用户显式启用的**工具级 Agent 授权与状态交接**，不要直接把所有六个 WeChat 工具无筛选交给网页或模型。
