# Profile、混合工作空间与无感认证

> 产品与架构决策：2026-10-27。**设计合同，不表示 PersonalProfile/可信绑定/浏览器直连已在 0.1.0 交付。**

## 1. 一个本机用户，多个 Profile，默认混合

- **ApplicationProfile V1**：有服务端的企业应用或个人 Web 服务发布应用实例身份、可信来源、服务地址和请求的能力；应用负责自己的账号、SSO 和业务鉴权。**保留已有 V1 Schema 与 CloudFile Rust Agent2 消费兼容。**
- **PersonalProfile（规划中的轻量本地配置）**：为个人已安装的 Agent、MCP、文件与通讯 Connector 提供默认本地工作空间；不要求 origin、企业 SSO、个人云账号或第二套服务端 JSON Schema。先落地最小本机配置，有实际需求再固定格式。
- **LocalBinding**：记录已受信应用、当前 OS 用户、本机已安装能力、用户连接器许可、Dev/Prod 安装绑定和真实 Host；由本机决定执行者，远端 Profile 不能指定任意 EXE、Host 或更新签名。
- **EffectiveProfile**：当前可使用的能力派生视图，不另存一份冲突配置。
- **BridgeTask**：Profile ID 记录来源/账号/任务归属，不是默认个人与企业的数据隔离墙。

**默认混合协作：**个人微信助手、Etech 元枢、CloudFile、Codex/WorkBuddy/Pi 使用同一个 Runtime 和共享的已授权能力池；不额外要求跨 Profile 许可。用户可选独立工作空间或隔离单个 Profile 的本地路由；此设置不是 OS 沙箱或企业 DLP。

## 2. 已登录 Chrome 到本机的无感认证

1. 用户已经通过 Etech、CloudFile 或其他 Web 应用登录。它们可以各自对接 Authentik、其他 OIDC/SAML、企业代理认证或既有 Session。
2. 已登录网页向**自身应用后端**请求针对本次对象/动作的授权；后端根据既有用户会话和业务权限签发限范围、短期操作票据。
3. CloudFile 文件操作优先复用现有 **V2 browser-scoped ticket**；不要新造一套 CloudFile 文件认证流程。
4. Chrome 扩展在受信 origin/消息校验后，经 Native Messaging 将票据交给现役 Agent2 或未来 Local Bridge，调用明确的已注册能力。
5. 实际文件/业务操作、ACL、提交与回执依然由 CloudFile 或企业应用服务端决定；Bridge 只负责本机正确路由与任务展示。

**无感 = 已登录时不重复输入密码，而不是不受控授权。** Bridge 不读取浏览器 Cookie、长期 Token、用户密码或制造一个新的 IdP。独立启动的 CLI/Agent 若没有现成浏览器受信授权，仍需走原应用允许的登录/授权流程。

| 应用 | 认证权威 | Bridge 的责任 |
|---|---|---|
| Etech | EAP 已有 Session、SSO、业务对象权限 | 用其后端授予的操作范围，不实现 Etech 登录 |
| CloudFile + Authentik | CloudFile 的 OIDC Session 与 ACL | 复用 CloudFile V2 文件票据及 Guard |
| 第三方企业 SSO | 对应企业应用接入自己的 IdP | 不要求 Bridge 支持每家 IdP 的密码流程 |
| 个人 Web 服务 | 原服务 OAuth/Session | 同样使用 ApplicationProfile 和原服务授权 |
| 纯本地通讯/Agent | OS 用户与用户安装的连接器授权 | PersonalProfile + LocalBinding，无企业登录 |

## 3. 混合不等于复制全部数据或绕过原授权

**共享：**已安装且真正获许可的工具、Agent/MCP、用户选定的数据交接、统一任务状态。

**保留原系统归属：**浏览器会话、企业 Token、CloudFile Journal/Guard/版本、通讯历史和发送回执、Agent 原会话/密钥。个人与企业跨工具交互没有额外的 Bridge 审批，但每个工具仍验证其自身权限与连接器许可。群消息、网页文本和模型输出属于输入数据，不能自行授权发送、删除或执行任意程序。

**不建设：**企业 DLP/数据分级/终端策略中心，也不试图绕过公司实际终端控制；Dev 与 Prod 的 Host、凭据、工作副本、Journal 和升级来源继续独立，只为执行正确性和数据保护。

## 4. 最小使用验证

在公司电脑同时登录 Etech/CloudFile 和个人微信，CloudFile Agent2 正常编辑文件，个人 Connector 在用户许可的聊天范围内读取/监听消息并触发 Agent；Bridge 能关联展示并将用户选择的信息交给有权调用的工具。不切换 Profile、不额外重复 SSO、不新建跨 Profile 审批。Bridge/Connector 缺失时，原文件和企业 Web 功能不受影响。

相关：[个人与企业场景](PRODUCT_POSITIONING_AND_USE_CASES.md)、[ApplicationProfile V1](APPLICATION_PROFILE_V1.md)、[通讯 Agent](AI_AGENT_AND_LOCAL_COMMUNICATION_V1.md)。
