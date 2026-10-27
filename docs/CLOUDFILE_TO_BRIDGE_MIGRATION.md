# CloudFile Agent → Generic Local Bridge — 并行演进及零破坏约束

状态：**分阶段接入方案，未启动生产切换**。第一原则：通用 Bridge 的平行开发不得改变任何已投入使用的 CloudFile 与上层应用的实际行为。

## 1. 三者关系与唯一依赖方向

```text
Enterprise Application --publishes--> Generic ApplicationProfile
          |                                   |
          +-- existing business APIs          v
          +-- CloudFile Hub/Server       Local Bridge
                  |                          |
                  +-- current file flow      +-- CloudFile adapter
                                             |     <--> CloudFile Agent2
                                             +-- optional Pi / MCP
```

**应用 → 配置提供方**；**Bridge → 通用本机运行时**；**CloudFile ↔ Bridge → 允许双向适配的文件能力**。

Bridge 不可硬编码上层应用名、项目字段、业务账号、员工组织、业务 API 端点，也不直接管理企业服务端权限。应用侧部署 Profile 与业务 Context；Bridge 只认识标准合同。

## 2. 必须保留的已用入口

以下是现有 CloudFile Agent2 文档记录的兼容界面，切换前以实际已安装版本再确认：

- Windows 生产 Native Messaging Host：`com.cloudfile.current_agent`；
- macOS 正式 Host：`com.cloudfile.agent2`；
- Rust 开发 Host：`com.cloudfile.agent22.dev`，开发与正式不可互相覆盖；
- Chrome 现有 `status`、`open_uri`、`task_summary`、`focus_task`、`query_local_file`、`query_local_folder`、`open_workspace` 与更新、配对消息；
- CloudFile 已有 SSO/登录 Cookie、Hub/Server URI、原生 Seafile 路由、普通/独占编辑权限及更新机制；
- 文件工作副本、Journal、Intent、Guard、Checkout/Checkin 和历史版本，特别是未完成及结果未知的任务。

新增 Profile 不修改上述协议，也不改变已有上层应用的配置键、登录逻辑、文件服务 URL、业务账号或文件权限。不得通过默认设置将生产 Host 切到新的 Bridge。

## 3. 演进步骤及回退

### A. 合同准备（现在）

- 在 Bridge 主仓引入`ApplicationProfile V1`、`BridgeTask V1` 和 Capability/MCP 合同；只增加离线校验、文档与可运行样例。
- 上层应用**自行**提供 Profile 示例和适配计划。部署尚未注册时，完全走旧 CloudFile 路径。
- CloudFile Agent2 与 Chrome 仍以现有调用为准；不因引入 Profile 而改变任何请求路由。

当前直连文件模式 **不是** Bridge 的子模式：应用可使用同一份 ApplicationProfile V1 向其现有 CloudFile Rust Agent 声明文件通道，而 Chrome / Native Host 仍按原路径运行。Agent2 对 Profile 的消费属于待实现的可选适配，不能因 Profile 格式统一而声称当前已完成注册或授权；安装 Bridge 不成为存量文件操作的前置条件。

未来接入后，配置 `profile_id` 与 `channels[].module=cloudfile` 保持一致；变化仅限本机受信绑定和新任务路由，不能由服务端 Profile 直接指定任意 Host 或可执行程序。正式升级的重点是**原任务继续由原 Journal 完成**，不在升级时将有效 Checkout 或未知发布复制到新执行者。

### B. 只读旁路（优先 DEV）

- 本机可信注册 Profile，并将其`profile_id` 与已确认的原 CloudFile 服务/用户绑定；不改变原配对与登录。
- Bridge 只读读取 CloudFile 任务/版本状态，不持有 Cookie、任意工作文件路径或独占锁操作；不能让旧 Agent2 任务因 Bridge 不在线而失败。
- Dev 和 Prod 安装身份及目录隔离。先在独立 Dev Extension/Host/测试账号上验证，不覆盖现有正式扩展的 Host Manifest。

### C. 受控转发（独立验收）

- 逐项增加现有 V2 已有能力的 Bridge IPC 适配：查看→普通编辑→独占编辑；请求仍由 Agent2 的受信票据、Journal、Guard、原生提交机制执行。
- 每个能力通过实际 Windows 文件/Chrome/Hub/Server 验证后，才能从旧入口切换到 Bridge 转发。切换失败必须能按任务回到**原既有入口**，不能在一个未知结果的写入操作中自动换 Host 重试。
- AI/企业任务可以旁路接入统一任务中心，但不得更改文件模块的完成判据。

### D. 客户端归口（满足完整替代后）

- 通用 Browser Extension / Local Bridge 统一安装与任务 UI；原正式 Host 名称可由兼容适配器继续服务，直至真实安装基数完成迁移。
- 当旧 Agent2 有进行中的文件任务时，继续由旧模块处理直至完成；即使 UI 改由 Bridge 展示，也不转写 Journal。
- 一次用户明确的受控升级，备份旧 Host Manifest、保留工作副本和凭据，健康检查失败立即恢复原入口。不能把“可启动”当成“文件版本已经安全发布”。

## 4. Dev/Prod 隔离与验收矩阵

| 维度 | DEV | PROD / 存量 |
|---|---|---|
| Profile | 独立稳定`profile_id` | 原位绑定稳定`profile_id` |
| Chrome Extension | 单独开发扩展 ID | 已安装正式扩展保持 |
| Native Host | `com.cloudfile.agent22.dev` | 当前生产 Host 不变 |
| 配置/Journal/工作副本/设备凭据 | 独立目录与身份 | 不修改原目录、任务和登录 |
| 程序升级 | Dev 发行信任根 | 既有正式发行签名根 |
| 回退 | 可以卸载 Bridge Dev 而无影响 | Bridge 不可用时仍可用旧文件入口 |

最低验收：
1. 不安装 Bridge 时，当前应用的登录、目录、文件下载、三种本地操作仍按现有版本行为运行；
2. 安装 Bridge Dev 后，正式 Chrome/Host/工作文件/SSO/权限均不变；
3. Bridge 只读任务摘要与 CloudFile 自有任务中心一致，含冲突、未知回执、Checkout；
4. Dev 两个 Profile/两套 Host 共存，不混任务或凭据；
5. Windows 首先验收；macOS、Linux 共享合同逐步补齐，但不得拖慢正式 Windows 文件流；
6. 正式转发需另立验收，不凭 Mock/CI 通过直接切生产。

## 5. 明确禁止

本合同阶段不启动数据迁移、SSO 重构、CloudFile Hub/Server API 改造、Chrome 默认 Host 接管、文件引擎复制、生产部署或 MCP 写入工具开放。只在验证确有新业务需求时增加必要能力适配。

下一步工作以一个跨应用 Profile + 一个只读 CloudFile 任务摘要为最小垂直切片；不要先做复杂模块市场或通用任务数据库。
