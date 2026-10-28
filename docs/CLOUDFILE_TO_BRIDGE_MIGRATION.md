# CloudFile 与 Local Bridge：两套产品长期独立，未来才考虑升级合并

**决策（2026-10-29）：现有 CloudFile 与新 Local Bridge 是两套独立产品栈，长期并行；没有默认自动合并、替换或切流。** 只有用户单独批准“升级合并”，且完成正式文件三模式、旧任务数据、Chrome/Host、签名升级与回退验收后，才允许改变这个状态。

## 产品身份及依赖（当前约束）

| | ① CloudFile 既有产品 | ② Local Bridge 新产品 |
| --- | --- | --- |
| 源码 | `cloudfile-local-agent2` **既有 dev** + `cloudfile-chrome-extension` | `local-bridge` + `local-bridge-chrome-extension` + 可选 `cloudfile-bridge-provider`（Agent2 Provider 独立分支） |
| Chrome | 已有 CloudFile 扩展，固定 ID `gocnpbhnbikcpnlafmodflddgnmfcjoc` | 新独立 Chrome 扩展，使用自己的实际安装/发行 ID；目前不假设已经固定商店 ID |
| 本机 Host | Windows `com.cloudfile.current_agent`；macOS 正式 `com.cloudfile.agent2`；当前 Rust DEV `com.cloudfile.agent22.dev` | `com.eaopen.browser_bridge`，独立 Manifest 和浏览器授权 |
| 文件状态 | Agent2 Tauri、File Core、Journal、Guard、工作副本、设备身份和签名更新 | 新 Provider 的**独立数据根** `CloudFileBridgeProvider`；Bridge 本身拥有独立的任务视图/注册/授权 |
| AI/通用 MCP | 不需要 Bridge 才能查看/编辑/独占文件 | Bridge + 新插件可独立使用 Codex/Pi/WorkBuddy/MCP；没有 CloudFile Provider 时文件功能不可用 |
| 安装升级 | 原 CloudFile 安装、Chrome Host/扩展更新、回退保持不变 | Bridge、新 Chrome 和 Provider 分别安装、升级、回退，不能写旧 Host/Journal/升级密钥 |

**通用代码可以复用，运行身份和本机数据不能自动共享。** 第二套的 Provider 从 Agent2 Rust Core 复用文件能力代码，**不调用已安装的第一套 Host，不读取其旧任务/工作副本/Journal/设备密钥**。双方都可以在各自通过服务端授权时调用 CloudFile Hub/Fileserver；云端服务相同不等于本机任务/身份相同。

### 两条长期保留的调用路径

```text
① CloudFile Web
      → 现有 CloudFile Chrome 插件
      → 原 CloudFile Native Host
      → Agent2 Tauri / CloudFile Core → 原工作副本 / Journal / Guard
      ↕ CloudFile Hub / Fileserver

② 通用 Web / AI 工具
      → 新 Local Bridge Chrome 插件
      → 独立 Local Bridge Native Host / MCP / HTTP
      → 用户主动启用的 cloudfile-bridge-provider（非必装）
      → 第二套的文件 Core / 私有工作区和任务
      ↕ CloudFile Hub / Fileserver
```

- **① 不依赖 ②。** 不装 Bridge、不装新 Chrome、Provider 崩溃时，第一套文件查看/编辑/独占、Tauri 和更新继续正常。
- **② 不把 ① 当自动回退。** 缺少 Provider、权限不符或执行结果未知时，第二套报告不可用/未知，不转给旧 CloudFile Host、不重新消费一次性票据、不从旧 Journal 取任务。
- 不同产品的 Chrome 扩展、Native Host、安装路径、数据根、发行信任分别维护。Windows `com.cloudfile.current_agent` 是旧 CloudFile 的既有固定入口，**禁止新 Bridge 接管**。
- 新 Chrome 插件目前可独立做复制/网页内容接力；即使本机尚未安装 Bridge，也不影响基础功能。但不能因此将它视为已经替代旧 CloudFile Chrome 的本地编辑能力。
- Bridge 可以保留现有 `cloudfile.status` **显式配置的旧组件只读诊断兼容项**，但**第二套的标准 CloudFile 文件接入只能走 `cloudfile-bridge-provider`**。不自动从第一套 Host Manifest 发现或路由新 Provider。

## 代码与运行时的依赖方向

企业应用提供 ApplicationProfile、业务身份与合法的 CloudFile 授权；Bridge 不能硬编码企业应用名称、员工信息、库业务逻辑或访问口令。CloudFile 文件 Core 是文件处理与权限/条件提交的 Owner，Bridge 只是通信、能力注册、任务展示与已授权请求分发。

- Bridge 核心保持轻量 Rust CLI/Native/MCP/Web V2，**不纳入 Tauri、Windows WebView2、CloudFile Journal/Guard、上传/升级引擎**。
- 第一套 Agent2 继续保留 Tauri/任务中心和原有安装器。
- 第二套 Provider 独立安装二进制 `cloudfile-bridge-provider`，当前只开放 `cloudfile_status` 与 `cloudfile_task_counts` 两个只读 MCP 工具。默认私有根为用户本地应用数据下 `CloudFileBridgeProvider`，可由本机部署使用 `CLOUDFILE_BRIDGE_PROVIDER_HOME` 指向独立绝对路径，**不读取 `CLOUDFILE_AGENT2_HOME`**。
- Bridge 当前 `provider add` 的路径被固定为已安装二进制位置。第二套 Provider 升级/路径稳定机制须由第二套自己解决，**不得改由读取第一套 CloudFile Host Manifest 获取二进制位置**。
- 通用 Provider MCP 工具不会自动授权给网页和模型。Provider 真正的文件操作未来仍需当前用户的 scoped ticket、Agent2 文件 Core 和 CloudFile Fileserver 终判；Bridge 的连接状态不是文件写入授权。
- 两套默认不共享任务索引；`BridgeTask` 以后汇总的是**第二套 Provider 的任务**。旧 Agent2 任务只在第一套 Tauri 可见。若以后要聚合或迁移旧数据，必须纳入单独升级合并的验收范围。

## Dev/Stable 与企业 DEV/PROD 的区别

**产品栈 ①/②、客户端发行渠道 Dev/Stable、服务端环境 DEV/PROD 是独立维度。** 不借“同一个企业 Profile”绕过 Host/扩展/数据根隔离。当前 Windows 旧 Agent2 的正式 Host 是固定槽位；并不意味着旧 Agent2 的 DEV/STABLE 已能同机无冲突并存。第二套 Provider 的正式独立安装/升级尚未发布，因此不得虚构完善的双渠道部署已经落地。

任何单个任务绑定原始产品栈、具体 Provider、来源及业务授权。请求超时、网络回执丢失、未知发布、活动 Checkout 时，**不能在两套之间自动切换/重试**。

## 近期和长期研发里程碑

1. **当前双轨**：第一套 Agent2 `dev` 和旧 CloudFile Chrome 独立开发、独立使用。第二套 Bridge `dev` 和新 Chrome 独立开发，Provider 在 Agent2 `feat/local-bridge-provider` 功能分支上做只读 MCP，**不合并第一套 `dev`**。
2. **第二套自我完善**：Provider 先完成独立状态、能力和任务摘要，再用第二套自身 scoped grant、Journal/Guard/工作区逐项开发本地查看 → 普通编辑 → 独占编辑。保持第一套不变。
3. **通用职责逐步靠向 Bridge**：第二套可以建设自己的可选任务 UI、通知、入口和发行服务，但 Tauri 不成为 Bridge Core 必需依赖；第一套 Tauri 与 Host 长期继续工作。是否共享代码以实际重复劳动为准。
4. **合并升级（未来单独决策，不在当前 Roadmap 内自动发生）**：明确最终 Chrome ID/Host/数据目录/发行信任选择；验证 Windows/macOS 真实文件三模式、SSO、原文件版本、Journal/未知 Intent、Checkout 续租、工作副本与用户应用规则、签名升级/回退，保留原任务执行者直至结清；经用户明确批准后才允许切入口或退役旧产品。

**不做：** 未经明确决策自动把原 CloudFile Chrome 升级为新 Chrome；Bridge 接管旧 Host； Provider 读取旧数据目录/旧设备密钥；将两套任务存入共同数据库；把 CloudFile 上传/Checkin 实现在 Bridge；删除已有用户工作副本或强制关闭有效 Checkout。

## 依据

- [第一套 CloudFile Agent2 既有兼容合同](https://github.com/eaopen/cloudfile-local-agent2/blob/dev/docs/CONTRACTS.md)。
- [第二套 CloudFile Provider 代码/文档分支](https://github.com/eaopen/cloudfile-local-agent2/tree/feat/local-bridge-provider)。
- [通用 BridgeTask 合同](BRIDGE_TASK_V1.md)：任务显示不代表文件执行账本。
- [新 Chrome 插件的迁移说明](https://github.com/eaopen/local-bridge-chrome-extension/blob/dev/docs/CLOUDFILE_MIGRATION.md)。
