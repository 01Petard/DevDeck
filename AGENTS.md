# AGENTS.md — DevDock 的 Codex / Work 执行规则

这是一个真实的 macOS 原生工具开发任务。开始修改前，请阅读 `docs/00_开始开发.md`、`docs/01_产品需求_PRD.md`、`docs/02_技术设计.md`、`docs/03_数据契约与错误码.md` 和当前阶段 `docs/05_Codex_分阶段任务.md`。

## MVP 边界

DevDock v0.1 只做：**macOS TCP LISTEN 扫描、进程元数据、搜索、端口收藏、回环 HTTP 健康检查、安全 SIGTERM、DMG 打包**。

严禁擅自增加 Docker、项目启动、Shell 终端、数据库、远程操作、多平台适配、AI、监控仪表盘、菜单栏、自动更新。

## 开发约定

- 先检查实际仓库代码和锁定版本，再修改；不臆造 Tauri/Rust API。
- 优先使用官方 Tauri 2 Vue/TS 模板，遵循已有项目结构。
- 系统操作在 Rust；Vue 负责交互。复杂职责只拆必要模块，不过度设计。
- 不允许 Shell 拼接命令、提权或在日志中输出敏感凭据。
- 不用模拟数据冒充原生真实结果；测试 fixture 与生产 UI 数据严格区分。
- 终止进程是危险操作：必须明确 UI 确认，并在 Rust 二次验证身份、只用 SIGTERM。
- 不得终止用户当前已有的重要服务。真实终止测试只操作本次任务创建的临时测试服务。
- 每阶段按 `docs/05_Codex_分阶段任务.md` 逐项实现并验证，通过后停止，不扩大范围。

## 验证和汇报

按需要运行 `pnpm build`、`cargo test`、`cargo check`、真机手工测试和 `pnpm tauri build --bundles dmg`。报告每项 PASS / FAIL / NOT RUN 及证据。没有 Mac 或未亲自执行打包，不得宣称 DMG 验证成功。

最终报告保持简短：已实现、变更文件、验证结果、遗留限制。
