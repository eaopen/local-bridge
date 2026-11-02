# 微信原生分享 / ZIP 导入 — Bridge 侧边界与 P1 交接设计（2026-11-02）

关联：`eaopen/wechat-provider` `feat/native-share-import`、`CoBeing/companion` `feat/native-share-import`。
跨仓契约（v1）：见 Companion 仓库 `NATIVE_SHARE_CONTRACT_2026-11-02.md` 的落地版本与 Provider `docs/NATIVE_EXPORT_IMPORT.md`。

## 结论（本轮 P0）

**Local Bridge 不需要新增微信业务代码，也不需要新增文件交接接口。** `dev` 上已有的通用能力刚好够用：

1. `mcp_provider::call(provider, tool, json_input)`：通用 MCP stdio Client。Companion 用它调用用户已注册的独立 WeChat Provider。
2. Provider 自行管理账号、SQLite、解析、来源标记；Bridge 不解析任何聊天格式、不持有聊天历史、不发消息。
3. 文件字节**不经过** MCP JSON：Provider 工具只接受一次性「授权令牌」，令牌由 Companion 原生进程在用户主动选择文件后写入 Provider 的私有授权目录；Bridge 只转发令牌与解析结果的小 JSON。

因此本轮 Local Bridge 只有文档变更，不改 Rust 代码、不加依赖、不加常驻服务、不加任务数据库。
这符合「能复用不重建」与「Bridge 只保留通用能力」：微信 ZIP 解析属于 Provider，聊天交互属于 Companion。

## 已验证 / 未验证（不夸大）

- 已验证：Bridge 的 `mcp_provider::call` 可作为 Companion → Provider 的传输层，调用新的 `wechat_native_export_preview` / `wechat_native_export_save`（合成 ZIP、真实 Provider 进程、MCP stdio 协议；见 Companion `scripts/e2e-native-share.py` 与两个仓库的测试）。
- 未验证：真实微信「合并转发 → 文件」实机操作、macOS Share Extension（P1，尚未实现）、Windows 分享入口、真实微信登录/数据库读取（本轮完全未触碰）。

## 安全边界

- 网页与 Agent 不获得 Provider 的 import 工具：Bridge 只从本机 CLI/Companion 原生命令调用，不把 Provider 工具暴露给 Chrome Native Host、Web V2 或 Agent 工具面。
- Provider 工具不接收路径与文件内容，只接收令牌；令牌一次性、10 分钟过期、同用户 0600、路径绑定。Bridge 不校验也不缓存令牌内容。
- 聊天文字是不可信数据：Provider 与 Companion 都不执行其中的指令，Companion 只把它作为数据标签交给已选 Agent。
- 保存、调用模型、发送是三个独立动作，必须分别由用户显式触发；本轮不实现发送。

## P1：macOS Share Extension 的最小交接（设计，未实现）

目标：微信「合并转发给第三方应用」→ 直接选 Companion → Companion 打开对应聊天预览，用户确认后才做 AI 操作。

最小方案（复刻 WeChatBridge 已验证的机制，不搬其应用代码）：

1. **由 Companion 打包 Share Extension**（`com.apple.share-services`，App Group 容器），不是新建独立产品。
2. Extension 只做「原子暂存 + 一次性请求」：
   - 每个批次 `Staging/<batch-id>.partial/items/<item-id>/<原文件名>`；
   - 全部附件落盘成功后再原子 rename 到 `Ready/<batch-id>/`，并写 `manifest.json`（batch id、来源动作、条目顺序、大小）；
   - `intent.json`（打开 Companion 预览 / 只保存文件）与 manifest 同批发布；**请求一次性消费**，过期未消费不执行。
3. 失败保留可重试记录（`.partial` + 清理任务），不静默重试、不重复处理已完成批次。
4. Companion 启动/激活时读取 `Ready/` 批次 → 调 Provider `wechat_native_export_preview` → 显示预览；不做模型调用，直到用户点击生成。
5. 无开发者证书时：Share Extension 需要签名才能被 `com.apple.share-services` 稳定加载（WeChatBridge 实测结论）。**不把证书申请作为 P0 交付门槛**：P0 保留 Companion 内 ZIP 文件选择/拖入入口，证书不可用时明确提示「系统分享入口未启用」，不改变微信登录环境、不重签微信、不注入、不自动重启。
6. Windows：只预留同一 manifest/批次接口（暂存目录 + 一次性 intent），不实现系统分享入口。

若未来 P1 需要 Bridge 参与（例如跨应用通用批次投递），只增加「用户已选择文件的通用暂存 + 一次性批次查询」两个通用接口，仍不出现微信字段、解析逻辑或聊天记录存储。

## 明确不做

- 不重建 Chat Copilot，不在 Bridge 保存/索引微信聊天，不新增发送账本、任务系统、认证系统或第二套 MCP 网关。
- 不因 ZIP 分享入口而要求 Bridge 获得微信数据库/SQLite 读取权限或完全磁盘访问权限。
- 不把 GUI 自动化成功当作消息送达。
