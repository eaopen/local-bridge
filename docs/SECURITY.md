# Security and Trust Rules

- **No arbitrary shell/EXE from enterprise manifest, Web page or AI prompt.**
- All MCP tools are explicitly implemented, read-only at 0.1.0, bounded in input size. Unknown methods/capabilities are rejected.
- CloudFile adapter executable location is set by the local user as an absolute path and must match the exact known filename. The AI cannot pass command names or process arguments.
- Only non-sensitive component metadata can be returned to MCP; never expose local paths, tokens, device keys or enterprise cookies.
- A Channel manifest has strict type, bounded cardinality/length, expiry, validated canonical HTTP(S) origin and same-origin relative Web paths. Validation is **not** authentication.
- Until a trusted fetch/authentication mechanism is implemented, configs are supplied via a local file solely for validation. Do NOT turn a remotely received unsigned manifest into executable capabilities. HTTP network attackers can modify unsigned configurations.
- Browser authorization and CloudFile file ACL enforcement remain in CloudFile/Etech; no permission is inferred from merely seeing a Channel.
- Native file-editing and version publication must call the existing Agent2 module, with its short-lived grants, Guard and durable Journal, not create a second upload engine.
- MCP stdio does not authenticate arbitrary local client processes by itself. Only connect it to user-approved AI apps and never grant powerful tools on the strength of this transport alone.
- Avoid default localhost web listeners, unrestricted filesystem access, remote code download, secrets in logs, silent Checkout release and auto-triggered write actions.

Security reports: use the repository maintainers' private contact method; do not post credentials or exploit payloads in a public issue.

## ApplicationProfile V1 / Dev 与 Prod

Profile JSON 的语法校验不等于来源认证、设备绑定或业务授权；`validate-profile` 永远不注册来源、不触发下载或执行。有效期、修订号与真正的受信 origin 必须在上线接入时再次验证。`update.source_id` 只能指向已安装并钉住签名公钥的更新来源，不接受服务端直接替换公钥。不同环境不能共用 Native Host、缓存/Journal、会话或 MCP 授权上下文。

通用 Bridge 不硬编码任何企业应用的接口或账号模型。CloudFile 的文件写入只能通过当前有效的服务端票据和现有 Agent2 Journal/Guard 执行。参见 [统一 Profile 合同](APPLICATION_PROFILE_V1.md)。
