# Roadmap — 通用 Profile 优先，非破坏性并行演进

**现状：** `0.1.0` 是只读 MCP + 离线旧 Channel 校验，CloudFile Agent2 负责真实文件编辑；本仓尚未实现可信 Profile 获取、通用任务中心、Pi RPC 或可写 CloudFile IPC。

**P0 · 通用合同（本 PR）**
- 固定 [ApplicationProfile V1](APPLICATION_PROFILE_V1.md)、[BridgeTask V1](BRIDGE_TASK_V1.md)、[Capability/MCP V1](CAPABILITY_AND_MCP_V1.md) 和 [CloudFile 非破坏迁移](CLOUDFILE_TO_BRIDGE_MIGRATION.md)；
- 增量增加 `validate-profile` **离线结构校验**；旧 `validate-channel` / read-only MCP 保留；
- 不在通用仓存放任何具体上层企业应用配置、地址、账号或业务规则。

**P1 · 可信 Profile + 只读任务旁路（DEV）**
- 建立受信应用注册、客户端 LocalBinding、Profile 缓存和 EffectiveProfile 计算；不得通过 Profile 启用任意 EXE、替换更新公钥；
- CloudFile Agent2 提供受信的只读任务摘要接口，Bridge 根据`profile_id`聚合任务；
- Dev 与 Prod Chrome/Host/Journal/凭据完全隔离；不接管任何生产文件入口。

**P2 · AI 与企业能力垂直切片**
- 一个通用企业 Web/AI 通道（应用自己提供 Profile，不让 Bridge 依赖其业务模型）；
- 可选 Pi RPC 与 Bridge MCP Client/Server 接入；先只读 MCP 任务/能力查询，写入工具要逐项业务授权；
- 一个真实跨应用非文件任务映射到 BridgeTask，和文件任务共同展示。

**P3 · CloudFile 受控文件转发**
- 经认证本机 IPC 顺序支持查看、普通编辑、独占编辑，复用 Agent2 Journal、Guard、原生条件发布；
- 每项在真实 Windows 普通用户+Chrome+CloudFile DEV 验收后再切流；macOS 同合同验收，Linux 先基础能力；
- 存量 Host、用户数据、SSO 和正在编辑的文件不迁移、不暗中修改。

**P4 · 通用浏览器扩展与 Bridge 统一入口**
- 确认所有旧真实调用均受支持后，可把产品入口归口为通用 Bridge；
- 保留旧正式 Host 的兼容转发和安全回退，直到已安装客户端经真实升级验证；
- CloudFile 长期作为文件能力模块，并不因此改变其业务权限或版本语义。

不做：重写 CloudFile 文件引擎、虚拟盘、独立 AI 模型网关、通用 RPA、远程 Shell/EXE、插件商店、大型任务数据库。未验收的目标不能写为已经实现。
