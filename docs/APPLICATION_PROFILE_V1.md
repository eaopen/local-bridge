# ApplicationProfile V1 — 通用应用配置合同

状态：**V1 合同与离线结构校验已提供；注册、下载、可信绑定和动态生效尚未实现**。这是跨 CloudFile Agent 与 Local Bridge 演进时必须保留的逻辑合同。

## 1. 角色边界

- **有服务端的企业或个人 Web 应用**拥有自己的身份、权限、对象和服务端配置，通过 ApplicationProfile 向本地请求能力。Local Bridge 不需要认识任何具体企业应用名称或业务数据库。
- **Local Bridge**负责 Profile 结构、受信应用注册、本机能力发现、任务聚合、用户确认、MCP 工具暴露和客户端生命周期。不实施企业项目权限，也不代发 CloudFile 文件版本。
- **CloudFile**与 Local Bridge 可双向集成：Bridge 发现并路由 CloudFile 文件能力；CloudFile Agent 可声明 Bridge 所需的能力与任务摘要。CloudFile 的文件会话/Journal/Guard 是文件操作的唯一事实源。

应用定制信息位于**应用侧**；通用 Schema 与验证器只存在于 Local Bridge。禁止把特定应用的固定服务地址、账号规则、项目字段和业务路由写入 Bridge 核心。

## 2. 唯一配置合同及本地生效

1. `ApplicationProfile` 是服务端发布的声明，不含账号 Token、更新签名公钥、本机 EXE 路径或任何可执行脚本。
2. `LocalBinding` 是本机安装/注册状态：`profile_id` → 受信 origin、当前用户和设备身份、已安装模块、开发/生产隔离目录、允许的发布源 ID。它不是另一套服务器 Profile。
3. `EffectiveProfile` 是运行时的**派生视图**：已受信的应用/本地个人配置 ∩ 实际可用模块 ∩ 当前操作的原系统授权 ∩ 本机用户许可。不要以它覆盖 ApplicationProfile，也不要持久化一份相互冲突的业务配置。
4. `TaskContext` 随单个任务传递业务上下文。Bridge 只处理明确的通用字段；额外的 `context` 为应用模块拥有的不可执行、不可信数据，不能直接解释成文件路径或 Shell 参数。

## 2.1 同一 Profile 支持“直连能力客户端 → 通用 Bridge”演进

ApplicationProfile **描述应用及其需要的能力，不描述本机执行程序**。一个企业应用可先由独立的文件 Agent 直接消费该 Profile 的文件通道；将来通用 Bridge 也消费完全相同的 Profile，再将文件请求交由同一文件能力提供者。

- `profile_id`、`origin`、`channels[].id`、`module`、`capabilities` 和语义保持稳定，`module=cloudfile` 等 ID 是**能力模块名称**，不是“Bridge 已安装/运行”的声明。
- 原有浏览器 → CloudFile Rust Agent 的执行路线可以继续存在，**无需现在安装 Local Bridge**；Profile V1 能否由某客户端实际消费，以该客户端已实现的解析、可信绑定和能力支持为准，不能把合同视为已有运行功能。
- 本机 `LocalBinding` 决定具体执行者（如现有 CloudFile Agent 直连，或以后 Bridge 的 CloudFile adapter）；**禁止在服务器 Profile 增加 `runtime`、Host 注册名或 EXE 路径来强迫切换**。
- 未来切换不要求应用发布第二份 Profile，也不默认迁移活动文件任务。既有文件 Journal 和 Guard 留在原执行引擎，新的任务在明确验收后切换路由；未知回执绝不跨运行时自动重放。

本约定适用于任何上层应用；**Local Bridge 核心不能包含企业特定适配名称或业务逻辑**。

## 2.2 个人工作空间与默认混合（设计约定，非新 V1 Schema）

- 默认个人工作空间（规划称 PersonalProfile）是同一 OS 用户选择的本地 Agent、MCP、文件和通讯 Connector 配置；没有必要为此要求企业 origin、SSO、服务器 expires_at 或 Bridge 云账号。首期**不新增第二套服务端 Profile Schema**，也不修改既有 ApplicationProfile V1 JSON 字段及 Agent2 消费合同。
- 同一个用户可以拥有多个 ApplicationProfile 和本机 PersonalProfile。Profile 标记服务来源、会话关联及任务归属，**不是默认个人/企业数据隔离边界**。默认允许使用其他 Profile 下已安装并获得本机与原服务授权的能力，无需 Bridge 特设跨 Profile 审批。
- 用户可自行选择独立工作空间或针对某 Profile 限制本机能力组合；此功能是可选路由设置，不是 OS 沙箱或企业 DLP。
- CloudFile、Etech 等原有业务对象权限、文件 Guard/Journal 继续由源系统决定。**企业 DEV/PROD Host、凭据、工作副本与更新根依旧隔离**，是防止错误路由和覆盖的运行约束，而非默认阻断个人与企业协作。

详见 [混合工作空间与认证](PROFILE_AUTH_AND_COLLABORATION.md)。

## 3. V1 JSON 字段

参见 [通用示例](../examples/application-profile.json)。JSON Schema 对应 `crates/local-bridge-core/src/profile.rs`，解析器默认拒绝未定义字段。

| 字段 | 要求 | 说明 |
|---|---|---|
| `schema_version` | 固定 1 | V1 格式主版本；不符合则拒绝，不静默转型 |
| `profile_id` | 稳定应用**实例** ID | 同一部署跨客户端/架构升级**保持不变**；Dev、Prod 各有独立 ID |
| `provider` / `environment` | 通用标识 | 应用提供者与环境；Bridge 不识别特定厂商 |
| `revision` / `expires_at` | 正整数 / Unix 秒 | 版本递增；受信生效时检查有效期（最大七天） |
| `application.id/name/logo_url` | 应用 ID、名称、Logo | Logo 可使用 HTTPS，内网 HTTP 仅限同源；Logo 不提供执行权限 |
| `origin` | 规范 HTTP(S) origin | 唯一来源身份基准；仅 JSON 校验不构成可信绑定 |
| `services` | 服务名 → HTTP(S) 地址 | 表示拟使用的业务 API 地址；跨域服务仍需单独可信授权 |
| `channels` | 最多 32 个 | `web` 为同源相对路由；`native` 为受信模块 ID；`capabilities` 仅表示**请求** |
| `update.track/source_id` | 发布轨道及本地钉住源 ID | 不能以远端 Profile URL 或公钥改变本机更新信任根 |

每个 Channel：`id`、`title`、`kind`（`web`/`native`）、`capabilities`、`enabled`（默认 true）、`optional`（默认 false）。`web` 必须给同源相对 `entry`；`native` 必须给 `module`，**不能**提供程序路径、参数或命令行。

当服务器未来需要新增字段或能力时，优先增加可选 Channel 与能力 ID；需要修改 V1 解析语义时显式引入新 `schema_version`，由服务端协商兼容版本。升级不能更换现有 `profile_id`。

## 4. 安全与升级规则

- **离线解析 ≠ 可信注册。** `local-bridge validate-profile examples/application-profile.json` 只返回 `shape_valid`，`trusted=false`、`activated=false`；示例失效时间不代表真实授权。
- 正式启用前需由用户/企业部署显式建立受信 origin 与本机身份绑定，从可信会话取得配置；HTTP 内网配置须额外有受信签名或等价完整性保护。缓存旧 Profile 仅供必要显示，不授予新的写权限。
- `capabilities` 不授予本地程序运行、读取文件、CloudFile 写回或 AI 工具调用权；每次操作均经过本机允许列表和拥有该业务数据的服务端授权。没有安装的模块必须显示不可用，不伪装成功。
- Profile 修改名称、Logo、菜单无需升级二进制；程序更新复用本机钉住的签名清单、健康检查和回退机制。活动 Checkout、未确认发布时不得静默替换文件引擎。
- 不同环境的 Profile、浏览器扩展、Native Host、工作区、凭据与更新源须隔离，不能凭网页请求自动切换正式/开发 Host。

## 5. V1 契约与现有运行代码

`ChannelManifest` 与旧 `validate-channel` 继续保留，属于已有 0.1.0 离线验证接口，**不自动迁移、不被悄悄重解释**。新增 ApplicationProfile 是目标统一合同；可信服务端获取、LocalBinding 存储、自动投影和任务执行为后续独立增量。

完成标准：通用 Profile 示例可离线校验；旧 CLI/MCP 调用与 CloudFile 现有网页入口行为不发生改变。
