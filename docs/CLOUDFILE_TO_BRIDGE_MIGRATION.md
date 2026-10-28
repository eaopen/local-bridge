# CloudFile Agent2 与 Local Bridge：两套连接入口，一份 CloudFile 文件数据

> 决策（2026-10-29，修正）：**旧 CloudFile Chrome/Agent2 和新 Local Bridge/Chrome 长期独立运行，直到明确批准升级合并；但同一个 CloudFile Agent2 安装仅有一套工作副本、Journal、Intent、Guard 和设备数据。** 文件数据共享是设计要求，不是两个产品的数据冲突。
> 参见 [Agent2 Provider 计划](https://github.com/eaopen/cloudfile-local-agent2/blob/feat/local-bridge-provider/docs/DUAL_STACK_COEXISTENCE.md)。

## 1. 两套入口的独立性

| | ① CloudFile 原产品链 | ② Local Bridge 新产品链 |
| --- | --- | --- |
| Chrome 扩展 | 原 `cloudfile-chrome-extension`，原固定 ID 不变 | 新 `local-bridge-chrome-extension`，独立扩展 ID |
| Native Host | Windows `com.cloudfile.current_agent`，macOS 正式 `com.cloudfile.agent2`，既有 DEV `com.cloudfile.agent22.dev` | `com.eaopen.browser_bridge`，独立 Host Manifest |
| 前端/任务 UI | Agent2 原 Tauri/托盘/任务中心继续工作 | Bridge 自身 AI/MCP/通用 UI 独立演进，Tauri 暂不迁移 |
| CloudFile 调用 | 原 CloudFile Chrome → 原 Agent2 Native Host | 新 Chrome → Bridge → `cloudfile-native-host --bridge-provider`（可选） |
| **文件数据** | Agent2 唯一文件目录和任务事实源 | **读取/调用同一个 Agent2 文件数据目录**，不建第二套 |
| 授权/版本 | CloudFile Hub/Fileserver + Agent2 原 scoped ticket、Journal/Guard | Bridge 只负责连接权限；文件操作须回到原 Agent2 引擎与原 CloudFile Server 终判 |
| 产品发行 | 旧 Chrome/Agent2 自己安装升级回退 | 新 Chrome/Bridge 独立发行；Provider 能力需与所用 Agent2 版本兼容 |

**两套独立的是 Chrome/Bridge 入口、Host 身份、通用任务/AI 配置、发行更新；不是两份 CloudFile 文件系统。** Bridge 的 MCP 注册、Web 站点授权和 AI Agent 任务继续放在 Bridge 自己的私有目录，CloudFile 文件数据只在 Agent2 目录中。

同一 OS 用户的同一个 Agent2 安装共用一份文件数据；Agent2 已有 Dev/Prod 或不同客户环境独立安装时，仍按它们原有的分隔规则，各自一份，不能跨环境错连。

### 两条长期保留的路线

```text
① CloudFile Web → 原 CloudFile Chrome → 原 CloudFile Native Host ─┐
                                                                ├─→ Agent2 Core/Desktop
② 通用 Web → 新 Chrome → Local Bridge → Agent2 Provider (MCP) ────┘           │
           └── 独立 AI Agent / 通讯 MCP                                          ▼
                                                                       唯一文件数据目录
                                                                       工作副本/Journal/Guard
                                                                                ↕
                                                                       CloudFile Hub/Server
```

Bridge/新 Chrome 缺失时，原 CloudFile 文件主链依旧能工作。Agent2 缺失时，新 Bridge/Chrome 的 AI/MCP、网页接力等通用能力仍能工作，但 CloudFile 文件 Provider 显示不可用。

## 2. Provider 已实现的代码范围（仅 Agent2 独立特性分支）

Agent2 `feat/local-bridge-provider` 在 **原 `cloudfile-native-host`** 内增加 `--bridge-provider` MCP stdio 模式：原 Chrome Native Messaging 帧协议和 Host 注册不变。它使用原 `storage::root()`（含 `CLOUDFILE_AGENT2_HOME` 与已有默认根），只读展示 `cloudfile_status` 和 `cloudfile_task_counts`。它不创建 `CloudFileBridgeProvider` 独立数据目录/第二份工作副本、Journal、密钥或 Checkout 状态；目前没有文件写操作。

本机用户显式注册安装了该模式的 Agent2 可执行文件：

```sh
local-bridge provider add cloudfile /absolute/path/to/cloudfile-native-host --bridge-provider
local-bridge provider tools cloudfile
local-bridge provider call cloudfile cloudfile_status '{}'
local-bridge provider call cloudfile cloudfile_task_counts '{}'
```

**开发/部署限制**：Provider 功能尚未合并旧 Agent2 `dev`；仅源码/CI 不表示已有在用版本可调用。Bridge 固定可执行路径，Agent2 采用版本目录升级；后续要提供稳定发现/重绑定，且确认两个进程确实解析到**同一个 Agent2 数据根**。不一致时应报告绑定失败，不能把新建的空根伪装成“旧任务已清空”。

现有 Bridge `cloudfile.status` 独立只读组件探测可保留兼容，但未来正式任务集成一律走经过协议协商的 CloudFile Provider，不复制私有文件状态数据。

## 3. 一份文件事实源，避免重复执行

- **Single Writer**：已有 Agent2 Desktop/Core 是文件任务唯一写入 Owner。Bridge 不运行自己的上传/Checkout/续租/Commit/Checkin；Provider 当前只读，以后写操作须进入原 Agent2 Desktop/私有 IPC 和原 `Journal/Intent` 锁。
- **Stable Task ID**：同一个 Agent2 文件任务从两个 Chrome 入口访问时仍使用原 `task_id`；BridgeTask 只作展示身份/授权视图，不生成第二份文件任务或数据库。
- **授权独立于目录共享**：看到了 Agent2 本机任务不等于获得该文件读写许可。每次新文件动作均须走合法用户会话/短期 scope ticket 和 Hub/Fileserver 的 ACL、File ID、Guard 检查，不能把 MCP 可连接当成 CloudFile 管理员权限。
- **未知结果不重试**：两套入口都不能在网络超时/原提交结果未知时跨 Host 创建新 Intent，必须查询原操作状态。Provider/Agent2 协议、Journal 版本不兼容时拒绝新的写操作。
- **不相互接管 Chrome Host**：新扩展只通过新 Bridge Host 连接；旧扩展继续使用旧 CloudFile Host。Browser 扩展 ID、原 Manifest 及旧用户安装路径均不修改。
- **更新/卸载保护**：两套连接软件可以分别更新，但共同的文件执行器是 Agent2。卸载 Bridge、新 Chrome 或取消 Provider 注册不得删除 Agent2 文件根；Agent2 自身升级/卸载保持原活动任务和备份保护。

## 4. 演进与未来合并

**P0**：只读 Provider 状态/任务计数，Tauri 不动。**P1**：Bridge 显示原 Agent2 脱敏任务摘要（同一事实源）。**P2**：按本地查看→普通编辑→独占编辑逐项将新入口受控请求送到 Agent2 唯一执行者；长操作采用持久化受理 + 原 operation 查询，不在 Bridge 同步阻塞几分钟。**P3**：按实际价值为 Bridge 增加可选通用任务 UI/通知，原 Tauri 仍可用。

**未来“升级合并”须单独批准**：明确最终 Chrome/Host/统一 UI/发行规则，完成 Windows/macOS 真实三模式、设备与文件权限、工作副本/Journals/Intent/Guard、签名升级/回退、已有用户任务接续和失败恢复验证后，才可切换旧入口或下线旧组件。在此之前两套 Chrome/Host 长期共存，但只有一个 Agent2 文件数据事实源。

## 5. 不做

不另建 CloudFile 文件工作区/第二份 Journal/Guard；不复制或搬迁 `cloudfile-core` 至 Bridge；不把 Tauri/WebView2 变为 Bridge Core 的必装依赖；不为“两个产品独立”重写文件存储；不在未获得文件授权时向模型开放 CloudFile 读写工具；不更换原 CloudFile 用户的插件和 Host。
