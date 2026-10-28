# 从 Agent2 提取 Local Bridge 公用基础能力（按真实价值分批）

2026-10-29 决策：**Local Bridge 是轻量本地连接底座，而非另一个 CloudFile Agent2。**
两套 Chrome/Native Host 独立；同一 CloudFile 安装的文件工作副本、Journal、Intent、Guard、文件写入 Owner **只有 Agent2 一份**。参见 [双入口/单文件数据目录](CLOUDFILE_TO_BRIDGE_MIGRATION.md)。

## 先迁哪一层

| Agent2 已验证经验 | Bridge 当前情况 | 处理决定 |
| --- | --- | --- |
| 用户级 Chrome Host 注册、所有权检查、备份、回退、卸载 | 只有 `native-host install [--replace]`，原写文件不可恢复 | **第 1 批落地**，只作用于 `com.eaopen.browser_bridge` |
| Agent2 `self-test` 可区分未配置/真正故障 | Bridge 只有简单 `status`，无安装/磁盘/Registry 诊断 | **第 1 批落地**，返回小型 JSON 健康结果 |
| 私有数据目录 + 原子写入 + 持久化/锁 | Bridge 的 inbox/Agent 任务/Origin 授权多个模块独立写文件 | 第 2 批按已发现的并发/中断问题收敛，优先共享可验证的小函数，不复制 Agent2 整个 `storage.rs` |
| 任务中心分组、挂起/未知/恢复 | Bridge 的 AI tasks 有 cursor，但 BridgeTask 聚合尚未实现 | 第 3 批只做已存在任务源的**只读**投影，不新建一份文件状态账本 |
| 开机自启、单实例锁、任务驻留 | Bridge MCP/CLI/Native Host 大部分为短进程、HTTP 服务可按需启动 | 有常驻实例用户场景后迁移；不复制 Agent2 的 Tauri 启动流程 |
| 签名清单、SHA-256、版本目录、健康检查、回退 | Bridge 还未形成正式发布/自动升级通道 | 另独立发版里程碑；借鉴原则和必要验证，不复制 Agent2 的密钥、Manifest 或更新源 |
| Tauri、File Core、CDC、Guard、Journal | 由 Agent2 承担 CloudFile 文件事务 | **不迁入 Bridge**；两个 Chrome 入口共用 Agent2 的唯一文件源 |

## 第 1 批实际变更：Host 生命周期与自检

- `local-bridge native-host status`：只读取本产品的 Manifest，区分缺失、损坏/外部注册、目标程序丢失、配置存在但**没有实测 Chrome GUI**。
- `native-host install <actual-extension-id>`：仅使用固定 Bridge Host 名称；已有相同配置可重做；不同配置需用户明确 `--replace`，写入前将完整旧 Manifest 留作 `.previous.json`，原位置同目录暂存写入后替换。
- `native-host rollback`：检查备份 Host 身份，再恢复原 Manifest；`uninstall` 仅移除当前 Bridge 自己的注册，不移除旧 CloudFile Host、不清理 AI/文件任务数据，也不擅自删除备份。
- Windows `HKCU` 使用现有 `reg.exe` 注册接口，检查是否已指向其他 Manifest；发现不同拥有者则拒绝接管。macOS/Linux 仍用原用户级 Chrome NativeMessagingHosts 文件目录。
- `self-test`：小型只读状态 + 临时文件读写检查，报告私有目录、Bridge Host 状态和 MCP Provider 注册数。**没装可选 Chrome Host 或 MCP Provider 不是故障**，但私有目录不可用或已安装的 Host 配置损坏应报告错误。
- 不默认开启 HTTP 监听、安装 TLS/CA、设置开机自启、调用 WorkBuddy/Codex/Pi、运行微信 RPA 或执行真实文件上传。

## 验收要求与实际限制

- **单元**：同配置重复安装、不同配置拒绝覆盖、显式备份替换、回退、卸载与外部 Host 拒绝删除。
- **跨进程**：在隔离用户目录运行真正 `local-bridge` 和 `local-bridge-native-host`，校验 Host 缺失仍可正常自检、原 Native Messaging/InBox 仍可用。
- **CI**：Windows/macOS/Linux Rustfmt、Clippy、test/build 全绿；不宣称真实 Chrome GUI/企业登录验收完成。
- 未把整套 Agent2 `storage/update/hostreg` 文件直接复制；只有实际需要的通用行为落入 Bridge。原 CloudFile Agent2 `dev`、原 Chrome 扩展、Agent2 Provider Draft PR #26 均不变。

## 下一步严格优先级

1. 先使用现有用户日志/故障验证 Host 安装、回退行为，若有 Windows Manifest/Registry 争用再做小范围修正。
2. 本地状态可靠性：只在发现并发更新或异常中断导致真实状态损坏时，引入统一原子写入/锁（关注 Origin grants、Agent task/Provider config）。不要一开始把所有目录迁移一遍。
3. 任务中心只读聚合 AI/Inbox/CloudFile Provider 状态，保持各自 Owner 与真实终态/未知语义；Agent2 文件路径和 Journal 不进 Bridge。
4. 常驻/托盘/自启与独立签名发版按实际使用需求进入后续批次；不把“迁移公用能力”变成大型平台重构。
