# WorkBuddy 原桌面账号本机验收实验

macOS / WorkBuddy 5.9.3 / 已登录桌面客户端 / Node 24+。这是固定任务实验，不是通用 AgentAdapter。

在仓库根目录主动运行：

```sh
cargo +1.90.0 run --example workbuddy_native_validation
```

每次创建新的私有临时目录，要求内置 CLI 写入固定内容。仅对指定文件、指定内容的 Write 请求批准一次，其他审批拒绝。Rust 随后独立读取文件核对。没有 `-y`，没有 CLI 登录或云端凭据读取；保留临时目录用于检查，不自动重放任务。不要把日志目录/config 作为验收附件上传。

只重新核验已完成任务（不再调用模型）：

```sh
cargo +1.90.0 run --example workbuddy_native_validation -- --verify /absolute/path/receipt.json
```

`workbuddyBroker.ts` 和 `workbuddyModels.ts` 参考 jiang198012/workbuddian 的同名源文件（2026-10-28 main），MIT 许可见 LICENSE。修改为本项目真实 client identity 和 `.ts` import。`run.mjs` 为本次实验驱动：loopback 认证代理 + 隔离内置 worker + ACP；只保存测试请求的回执，不保存桌面 ticket 或云端认证材料。

内部协议无稳定兼容承诺。启动配置请求 400、其他系统版本、长期运行、故障/取消清理、Windows、技能继承和计费差额未验收。此脚本只用于显式本机验证，不能直接开放给网页输入。
