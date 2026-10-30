# Local Bridge / WeChat Provider 边界、macOS 权限与大规模历史 P0–P2

**日期：** 2026-10-30。**状态：** 最新代码复核 + 可执行改造规划。不是已经实机验收的实现。
**源码基线：** Local Bridge `dev` @ `0f47a72`；WeChat Provider `dev` @ `db71b65`，以及用户新推送的 **未合并** `feat/wechat-mcp-direct-reader` @ `d1cff90`。以分支代码为准，不把未合并功能列入 Provider dev 已交付。

## 决策 1 — WeChat 数据和权限归 Provider，Bridge 只消费 MCP

- Bridge 核心不包含微信数据库路径、密钥、SQLCipher/WCDB 查询、UI 自动化、索引、聊天记录持久化，也**不得要求 Full Disk Access（FDA）**。
- 独立 `wechat-provider` 管理 read: `local_index / structured_db / desktop_ui`、可选 GreenBubbles / Wechat-MCP Reader、索引、source/account identities、IPC、macOS TCC；send: 已有 GUI 适配及 `send_tasks`，**独立于 read**。
- Chat Copilot 只消费用户选择的限定会话和片段，通过已有 Agent 生成/复制，发送后置；不能在 Agent prompt、网页页面或聊天正文中指定底层数据库路径/命令。
- **不要把两个微信数据库 Reader 当成两个 Provider/MCP Server**：WeChat Provider 已有六工具入口及 `source=structured_db` 抽象，Reader 是它的后端。后续可为重型历史索引加有界独立 CLI，但只保留**一个对外可用的微信 MCP**。

### 当前 FDA 问题：路径不是唯一原因，TCC 追踪负责进程

`d1cff90` 新增 `wechat_mcp_worker.py` 作为 WeChat Provider 子进程；当前 Bridge `mcp_provider::call` 经 stdio **由 Bridge 启动 Provider**，Provider 再启动读取 worker。于是原始微信容器的访问可能被 macOS 归给 Bridge Desktop Dev，而不是独立 Python Provider。新分支的 `docs/WECHAT_MCP_READER.md` 也明写“授权实际运行的 Bridge 应用”，**与目标边界不一致**。

**仅将错误文案改为“授权 Provider”不足以解决 TCC**。正确实现路径：
1. 一个**稳定安装和签名身份的 WeChat Provider.app / 本机原生 helper**。专有 Bundle ID，连续升级时保持代码签名/指定要求稳定；尽量不要把随开发热更新的 Python 解释器作为授权主体。
2. 独立由用户显式启用的**同用户 LaunchAgent** 管理该 Provider，而非 Bridge 的子进程；按 Apple 官方 TCC/launchd 规则正确配置 `AssociatedBundleIdentifiers` 或系统支持的 `SMAppService`。这里的“系统负责代码”要在实际 macOS 版本上验证，不能仅因 LaunchAgent 存在就声称权限独立。
3. **同用户私有 Unix domain socket**（runtime 目录 0700，Socket 0600；macOS peer UID 验证）；Bridge 侧用户注册的固定 stdio MCP 代理只连 socket、转发 MCP JSON-RPC，**永不打开微信 DB**。Provider 的 socket 服务只向已授权本地客户端暴露最小读取工具；原发送另需可见的人机确认。
4. **最小开发路径**：先以独立 Provider 的只读 IPC 服务 + Bridge 固定代理验证真实 TCC 归属。不要求一次性做 Tauri 配置 UI、进程守护、远程 API、完整 installer/自动升级；当前手动粘贴/Provider 索引作为回退。
5. 缺少授权文件、已授权快照、Reader/keys 时返回 `not_configured` / `source_access_denied`，不调用取钥、退出/重启微信、重签或调试附加。**不要自动授予 FDA**，用户必须自己在系统设置中批准。优先使用已授权导出/快照，从而尽可能避免 FDA。

**实机验收门禁**：
- 取此前已稳定授权的真实账号 **只读查询** 10 条，明确区分 TCC 授权主体与 SDK/Provider/Bridge 进程；不输出正文到日志。
- 暂时移除 Bridge FDA 后，分别验证 Provider 独立服务成功读取、Bridge 经 socket 代理成功读取；更换 Bridge Dev 二进制后重复测试，**不需要再次给 Bridge 授权**。
- Provider 新版本保持同一签名身份，测试重启后的读取状态；验证 socket 仅同用户可连接，拒绝缺失/错误来源身份。
- 微信 App 仍是原版、登录状态不改变。此前两次重新登录风险仍有效，**任何首次实机验证需另外明确批准**；没有批准只做合成数据/静态检查。
- macOS 不同版本 TCC 实际表现有差异，记录安装/签名/授权归属，未通过不得宣称“永久免权限”。

Apple 核实资料：[Accessing files from the macOS App Sandbox](https://developer.apple.com/documentation/security/accessing-files-from-the-macos-app-sandbox)；[TN3179 - responsible code and AssociatedBundleIdentifiers](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)；[TCC/Core OS 论坛](https://developer.apple.com/forums/topics/app-and-system-services/app-and-system-services-core-os)。

## 决策 2 — 「帮我聊」是人际沟通副驾驶，不是通用微信数据管理页

现有 `chat_copilot.rs` 有 `understand/reply/reflect`、`copy`、真实 Agent、事实/推测/未知；`chat-source.mjs` 已有结构化聊天分页、勾选、导入上下文。**继续复用**，不要做第二个聊天模型框架。

**P0 小改动**：
- 用户选择 **场景**：日常、朋友/同学、同事/客户、家人、刚认识、伴侣、多人群聊；这是本人指定的提示，不是 AI 计算出的永久关系标签。
- 模式独立：理解 / 回复 / 理清自己。关系影响表达策略：工作注重准确与边界、朋友自然、刚认识低压力、亲密关系尊重与修复、群聊按发言者总结。不要推断对方“肯定喜欢”、不要机械升温。
- 人类可读的会话显示名用于 prompt，保留稳定会话 ID 用于底层只读查询；不把稳定 ID 假冒 GUI 发送目标。
- 搜索现有聊天最多读有限页，避免一个输入动作展开上万会话的全库扫描。长期改为 Provider 的后端会话名称过滤或专门历史索引搜索，而不是 UI 无限翻页。
- 展示候选后操作链：编辑、复制、原 App 手动发送、自然/太油/太冷/不像我/越界反馈；不自动发送。已由 Provider 可靠验证 sender 后再实现显式确认的本机发送。
- 下一版可用快捷键/选中文本在微信窗口侧边唤起已有 Desktop UI。**不强制屏幕录制/OCR**，不因微信 UI/权限失效而失去手动复制流程。
- 不持久化原聊天、不跨工作/私人 workspace 共享第三方内容；可选长期记忆仍是用户显式保存的摘要（以后给 CoBeing/私人心舱）。

## 决策 3 — 100+ GB 历史：结构化检索是主线，不让大模型读全库

**事实：** 百 GB 通常包含媒体/附件，不能等同于百 GB 文本；必须先按账户、分库统计文字/非文本量，不能先做全量解密复制/导出。当前 `d1cff90` 的 `messages.search` 是最近 **100 条候选解码后扫描**（每个相关分库），**不是全库全文索引**。`messages.list` 每个分库都查一次并合并排序；在极多分库/多年历史和大量并发查询下会退化。它适合“近期指定会话的有限历史”，**不适合作为百 GB 分类引擎**。

**建议一条非破坏性的历史处理链**：

```
原微信数据（只读、已有账号授权/一致快照）
    ↓ 按账号/分库/时间/会话 + 稳定消息 ID 分页、续传
可选派生索引（Provider 自有 SQLite：元数据 + 选定文本 + FTS5）
    ↓ 先按规则筛选，剔除重复、系统噪音和无实际文字的媒体
会话 / 时间段 / 话题分块（每段保留来源 message IDs）
    ↓ 用户显式开启的本地模型（可复用已安装 LM Studio / Ollama）
主题标签 + 待办/约定/决定/关系片段 + 事件摘要
    ↓ 查询时先元数据/FTS，后检索少量摘要/原文证据
Chat Copilot：带来源和证据的简短记忆上下文（用户选择后）
```

**索引工程原则**：
- 归 WeChat Provider / 独立历史 worker 所有，不能由 Local Bridge 持有微信 DB 或另一份私聊原文库。若现有 `wechat.db` 的写入/发送延迟受影响，可选用**同一 Provider 管理的独立衍生 `history-index.db`**，不是再造一份微信数据库。
- 仅存文本、ID、类型、会话、发送者、时间戳、导入时间、只读媒体引用与来源健康。媒体/附件不自动提取、解密、复制或向 AI 投喂。结构化来源的 identity 有效性必须随账号/数据集变化核验。
- 源头增量游标：`(source_namespace, account, shard, conversation, timestamp, message_id)`，记录 checkpoint、已处理页与阶段状态。允许重复一小段重扫，按稳定复合 ID 幂等。原始 DB 更新时保留撤回/修改的事件语义，不能用 INSERT IGNORE 静默吞冲突。
- 单批有界读取/写入，例如 1000～5000 行、上限约 8–32 MB（作为初始验收参数，不是固定性能承诺）；短事务、崩溃后续跑、暂停/恢复。边扫描边存索引，不能在 Python 中把所有历史加载到内存。
- SQLite FTS5 优先：仅索引必要文本，中文可考虑 trigram，但**短于 3 字的词需要特别处理**；先评估索引大小与召回，再决定是否升级全文引擎。不默认引入 Meilisearch/向量数据库。针对超短关键词不得悄悄执行全库 LIKE 扫描。
- 实时问答先从 index 取少量候选，返回消息来源、时间、可靠性、截断/覆盖状态；生成的摘要只能作为辅助，不替代原消息证据。
- 批处理独立于 MCP 交互请求，避免 90 秒 MCP 超时；按需暂停、增量刷新，不因历史任务导致本机微信和聊天助手卡顿。

**模型整理：两级轻量化，不对每条消息跑一次 LLM**：
1. 规则/元数据先分段：按会话、日期、上下文间断、参与者与简单主题关键词切分；过滤系统通知和无文本记录，标记语音/图片为“未解析”。
2. 本地模型仅处理用户选中的有价值段落：给出多标签（工作/项目、朋友、家庭、亲密关系、生活事务、兴趣/知识、财务事项、噪音/待归档等）；另标注 `action / decision / commitment / followup`，提取明确出现的日期、人物指代和可核实引用；不确定时输出未知。
3. 对同一会话按**事件/周/月→会话摘要**进行层级聚合。优先保留下一步有用的“待办、约定、共同话题、未解决问题”，而不是长期人格推断。模型/提示版本、来源消息范围和用户修改记录决定是否需要重算；不重新遍历全部历史。
4. 本地模型作为**可选推理源**，优先复用用户已有 LM Studio 本机 API 或同等本地服务，要求实际绑定 `127.0.0.1`、明确推理范围、可暂停/删除；无模型也能全文检索与规则分类。不要因为这项需求在 Bridge 中搭建新的 AI 服务平台。
5. 默认不把任何非公开私聊丢给远端 Agent；需要远端时必须让用户看到具体范围并单独选择。

**分类维度不是单一“工作/私人”**：一段群聊可以同时是“项目 + 朋友 + 待办”，因此采用多标签，且“主题、要行动的事项、时间、敏感度、证据来源”分开。AI 猜测不得写成确定的个人标签；允许用户批量删除衍生摘要或排除某个会话。

### 交付步骤

**P0a（Bridge 当前可改）**：修掉“要求给 Bridge Dev FDA”的错误授权指导；引入用户显式选择的七种关系场景，调整 prompt 与 UI；可读名称与稳定 ID 分离；限制搜索 UI 的单次分页次数。现有代码路径：`chat_source.rs`、`chat_copilot.rs`、`chat-source.mjs`、`chat-copilot.mjs`、`index.html`。

**P0b（WeChat Provider 独立仓实施）**：审阅 `d1cff90` 并基于其 SDK Reader 做“稳定身份独立进程 → 同用户 IPC → MCP 代理”最小门禁，先合成数据验证再在用户明确批准的 Mac 验证。更新新分支里误导要求给 Bridge 授权的 `docs/WECHAT_MCP_READER.md`；未经验收不要将此能力列为可发布。

**P1（历史索引）**：先选定一个账号/少量会话、真实数据结构分析，再开发有界续跑的文本索引和范围搜索，验证索引大小、内存、召回和崩溃恢复。首轮不做媒体解析或全库 AI。

**P2（本地分类）**：在 P1 有可核查证据后，对选定历史段落跑本地模型的多标签/行动项/按时间摘要，生成可撤销的二级索引；Chat Copilot 用户选择是否附带。再考虑批量后台运行、跨 Soul 或 CoBeing。

### 最小验收及不做项

- Bridge 在**无 FDA**、Provider 未配置、Mac 被拒绝时仍可完成纯文本「帮我聊」；不自动重试未确认的发送。
- 独立 Provider FDA 归属的成功只能靠实际 Mac 测试确认；**不能以工具发现/CI 为准**。
- 历史验证要记录：读取总量/耗时/峰值内存/索引增长、消息去重、查询延迟、精确引用和恢复进度；从小范围逐步扩张到百万、千万记录/百 GB，不把小合成样本当真实负载。
- 不重复取钥/重签微信、不重建聊天平台、不增加任何权限绕过、自动批量私信、第二微信 MCP/多层注册框架、默认全库向云端上传或后台无限运行。
