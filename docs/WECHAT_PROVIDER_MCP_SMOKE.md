# WeChat Provider 实际 MCP 传输验证

这个可选 CI 验证的不是微信自动化，而是一个真正的、相互独立的 Python MCP Server 通过 stdio 被 Rust Local Bridge 调用的能力。

- 启动来自 [eaopen/wechat-provider](https://github.com/eaopen/wechat-provider) 的真实 `wechat-provider serve`（不是自制 JSON mock）；
- `local-bridge provider add` 保存当前用户配置，`provider tools` 协商并取得 6 个工具；
- 经 `tools/call` 调用 `wechat_status` 和空索引 `wechat_search`，校验结果；
- 对未知工具拒绝调用；没有微信账号、GUI、私聊读取或实际发消息。
- Local Bridge/WeChat Provider 仍可单独安装；本 CI 是可选互操作验证，不进入标准 Rust 多平台 CI 阻断项。

手动运行 GitHub Actions 的 **WeChat Provider MCP integration smoke**，或在修改本集成工作流和 MCP 客户端时自动进行。这种验证通过也**不表示 Windows/macOS 的微信 4.x UI 兼容、实际发送或接收方身份已经验收**，相关实机任务仍由 `wechat-provider` Issues #1/#2 跟踪。
