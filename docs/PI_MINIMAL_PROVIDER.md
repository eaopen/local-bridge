# Pi 最小可用接入：用户自己的 API / 模型，而非模型路由平台

**定位（2026-10-28）：** 这是**已配置 Pi 用户的可选使用指引**，不是要求每个人再办理模型账号的安装步骤，更不是企业 Web 必须走的默认 Agent。企业已有 AI 服务/API 时由企业应用后端负责模型、认证、额度和费用，Bridge 只连接获准的本地能力。个人优先复用 Codex/WorkBuddy 等原有登录；没有 Pi 模型 API 时直接使用现有 Chrome Web AI Relay 手动接力。只有明确需要自动调用 Pi、且用户自己已有或愿意配置官方模型 API 时，才使用下列可选 CLI。

## 1. 先在 Pi 自己的设置里连接模型

当前 [Pi 官方 Providers](https://pi.dev/docs/latest/providers) 支持 DeepSeek、Google Gemini、Groq、OpenRouter、OpenCode Zen/Go 及其他 Provider 的独立 API Key；也支持部分原有订阅登录。**先用 Pi 自己的 `/login` 或运行该进程的环境变量配置，不要在 Local Bridge 中再增加模型 API Key、路由规则和用户账号仓库。**

```sh
pi --version
pi
# 交互界面中选择 /login，再选择 DeepSeek 等已实际可用的提供商
# 或由用户自己的 shell/安全凭据管理器设置 DEEPSEEK_API_KEY
# 在 Pi 里用 /model 选模型，或在认证完成后查询实际模型 ID：
pi --list-models deepseek
```

在**明确知道当前模型 ID** 且 Pi 的原始连接已就绪后，运行本地 Bridge 的最小路径：

```sh
local-bridge agent pi deepseek <pi-model-id> "请把这段已选中的资料总结成三个要点"
# 支持从标准输入交接长内容，避免 shell 历史记录保留材料：
printf '%s' '一段用户批准的测试文本' | local-bridge agent pi deepseek <pi-model-id> -
```

Bridge 会调用 `pi --print --no-session --no-tools --no-extensions --no-skills --no-prompt-templates --no-context-files`，将输入通过标准输入交给 Pi。**这只是一次无工具副作用的模型任务**：不运行本机 shell、不允许模型改文件或发消息、不自动注册 MCP、不会复用 Pi 旧会话。只有 Pi 正常退出且返回非空文本才标记 `completed`。这不是完整的文件操作 AgentAdapter，也不假定任何模型真的零成本。

### 为什么不加 Local Bridge 的 API Key 页面？

API Key、OAuth、余额、模型列表、模型费用等均由 Pi 和各 Provider 已有功能负责；Bridge 管一次任务的调用/结果即可，避免额外配置、重复密钥存储及 9Router 式调度。可以在 Pi 中改一次模型或 API，Bridge 仅需要调用时指定对应 Provider/Model。

- **可选示例：DeepSeek API**，通常低成本，但需要用户主动开通 API Key/确认余额；**不是所有 Local Bridge 用户的默认配置**。准确模型 ID/价格以 Provider 和 Pi 实际目录为准。
- **可选：Gemini API 有条件的免费层、Groq 等已有额度。** 免费模型、额度、地域和数据处理条款以各自实际账户为准。
- **OpenCode 免费模型：** OpenCode 官方明确免费模型只在 OpenCode 内部提供。Pi 作为外部 API harness 访问这类免费模型会返回 403；即便 Pi 支持 `OPENCODE_API_KEY`，也不代表免费模型可以外部调用。若要用免费的 OpenCode，先独立使用 OpenCode 正式产品，不在 Bridge 中模拟调用或绕过厂商限制。
- **无任何 API Key：** 现有 [Web AI Relay](WEB_AI_RELAY_V1.md) 保留 ChatGPT/Gemini/Claude 等网页聊天的**人工复制 → 对话 → 带回结果**，不依赖 Pi；Web Chat 不是 Local Bridge 或 Pi 的自动模型 API。

## 2. 实施范围与下一阶段

现有 V1 仅保留可选的一次性 Pi CLI，不再把为 Pi 增加 API Key 配置或模型选择界面列为近期产品任务。**企业已有 AI API，无需经过 Pi；个人没有 Pi API 也不影响 Chrome Web AI Relay 和 Bridge 的其它能力。** V1 不新增 Provider 配置 Schema、模型路由、自动降级或计费层。缺少 Pi 程序/模型/认证时必须返回不可用；绝不改用某个历史 9Router 配置或静默转向云端 Chat。用户明确需要 Pi 时才指定 Provider/Model。

**验收还缺一条独立 API 的真实调用：** 当前可连接开发 Mac 上 Pi CLI 已安装，但未发现 DeepSeek/OpenCode/Gemini 直连凭据，因此不能声称已验证 Pi + DeepSeek。此前以用户已有 9Router 扩展跑通的 Pi RPC 是另一条独立验证记录。等用户自身 Provider API 可用后，用完全虚构的文本做一次任务，确认最终响应和用量符合期望，再决定是否值得做 Pi RPC、工具审批和 Web Agent 自动联动。

相关：[Agent 验收矩阵](AGENT_PROVIDER_VALIDATION.md) · [Web AI Relay](WEB_AI_RELAY_V1.md)。
