# 05｜Codex / ChatGPT Work 分阶段开发任务

> 用法：将本包所有文件放入仓库，交给 Codex/Work 执行。每阶段独立执行，满足“验收”后再提交并进入下一阶段。以下是明确授权的局部工程任务，原则上不需每阶段再次索取设计许可。

## 全局执行约束

- 使用仓库中的 `AGENTS.md`、`docs/01_产品需求_PRD.md`、`docs/02_技术设计.md`、`docs/03_数据契约与错误码.md` 为主要规范。
- 目标是完整可运行、可安装的 macOS 应用，不是 HTML 静态 Demo，不允许真实页面使用模拟端口数据。
- 功能以 v0.1 冻结范围为准，不自动开发后续版本。
- 端口扫描、进程终止只能操作或验证真实 Mac；非 Mac 环境只能完成代码和可执行的单元测试，不能报告原生功能已验证。
- 为避免误杀，不得终止现有重要服务；验证终止仅针对本任务自行启动、用户明确授权的临时测试进程。
- 每阶段只需修复本次修改导致的错误。无须新增复杂抽象层、大规模重构或全面文档翻新。

---

## P0｜工程初始化

**前置：** 空仓库或尚无 Tauri 工程。  **产出：** 可以启动的 `.app` 开发窗口与干净的工程骨架。

**Codex 指令：**

```text
在当前仓库构建 DevDock macOS 桌面应用基础工程。先检查目录，存在工程则沿用，不覆盖业务代码。
采用 Tauri 2 + Vue 3 + TypeScript + Vite + pnpm + Rust，初始化官方兼容模板。配置应用名 DevDock、bundle identifier top.bugstack.devdock、窗口初始 1000x680、最小 780x520，默认系统主题。
清理模板演示内容，只保留应用窗口与基础样式。建立 docs 规范指定的 src/types/devdock.ts、services/native.ts 等必要空接口，不预先新增空抽象层。不要添加 mock 服务、后端、数据库、路由、Pinia。
验证 pnpm build、cargo check、pnpm tauri dev（可运行的 macOS 环境）。提交一个可运行的最小工程。
完成时写明改动、验证证据和无法验证的事项。
```

**验收：** `pnpm tauri dev` 能启动干净窗口，`pnpm build` 和 `(cd src-tauri && cargo check)` 通过；无模板欢迎页。

---

## P1｜lsof 端口扫描与进程信息

**前置：** P0。  **产出：** `list_listeners` 真实 IPC 命令及测试。

**Codex 指令：**

```text
按 docs/02_技术设计.md 和 docs/03_数据契约与错误码.md 实现 macOS TCP LISTEN 端口扫描。
固定执行系统 lsof，不使用 shell。采用 -F0pcftn 机器输出；正确解析 PID/进程名/文件类型/IP版本/Socket 名称、处理 IPv4 IPv6 通配地址、多 socket、重复记录、空结果、超时、无权限和进程中途退出。扫描 5 秒超时，限制输出大小。
使用 sysinfo 一次刷新获得可用的 processStartTime、exe path 等；不可获得的字段为 null。严禁推测项目名和启动时间。
注册 list_listeners Command；Rust 与 TS 按 camelCase 契约一致。给解析器提供 fixture 单元测试，覆盖不同字段分组、IPv6、重复端口、格式不合法、空数据、命令失败的映射。
Mac 上自行启动临时服务 python3 -m http.server 18765 --bind 127.0.0.1，并与 lsof 输出核对。非 Mac 不声明真实扫描成功。
仅实现当前功能，不触及健康检测和终止命令。
```

**验收：** `cargo test`、`cargo check`；真实 Mac 能发现 18765 并正确显示 PID；重扫消失正确；扫描失败不是空列表。

---

## P2｜主界面与列表详情

**前置：** P1。 **产出：** 真实可用的单窗口 UI。

**Codex 指令：**

```text
实现 DevDock Vue 3 单页面主窗口。顶部搜索和刷新，左侧监听 Socket 列表，右侧当前选中监听详情。所有数据从 list_listeners 读取，无 mock。
支持端口/PID/进程名搜索；端口升序排序；点击候选行显示进程名、PID、监听地址、启动时间、可执行文件路径。不可读显示“不可用”。启动自动扫描；刷新防抖/防并发；窗口重新聚焦时合理节流刷新。
必须区分加载中、扫描失败、真实无监听、过期快照；刷新后已消失的进程不得保持可操作的旧选中状态。
使用简洁 macOS 工具型 UI，系统深浅主题，不引入 Router、Pinia 或图表。
执行 pnpm build；Mac 上手动验证测试端口出现/消失和搜索。
```

**验收：** 展示真实监听；搜索准确；失败/空状态准确；刷新不遗留危险操作目标。

---

## P3｜端口收藏与回环 HTTP 探测

**前置：** P2。 **产出：** 可重启保留收藏、GET 健康检测。

**Codex 指令：**

```text
实现收藏端口和健康检测。收藏使用 Tauri Store，保存 settings: { schemaVersion: 1, favorites: [...] }；端口唯一，别名 trim 后 <=60 字，允许收藏未启动端口。打开/编辑/保存/取消收藏必须可靠；保存失败告知用户，损坏的配置不可静默覆盖。
Rust 实现 check_http_health({url})，固定 GET，超时 3 秒，只读响应头，使用 reqwest 禁用代理、禁用重定向。URL 只支持 localhost、127.0.0.1、::1 的 http/https 且不得含用户信息，并确保 localhost 只能连本机回环。保存收藏时 URL 端口必须等于收藏端口。禁止任意 URL、任意 Header、认证、cookie、自动遍历端口。
2xx healthy；非 2xx http_error；网络错误 network_error；超时 timeout。返回状态码、耗时和时间。健康检查最多并发 4，探测仅限用户显式配置的 URL。
增加配置/URL 校验、HTTP 测试。Mac 上分别验证 200、404、连接拒绝、超时、301 不重定向及收藏重启保存。
```

**验收：** 收藏持久化；未监听收藏不丢失；本机 URL 正常探测；外部 URL 无法探测；3xx 不跟随；无配置时不发请求。

---

## P4｜安全 SIGTERM 操作

**前置：** P3。 **产出：** 安全明确的进程终止能力。

**Codex 指令：**

```text
只实现对当前列表内真实进程的 SIGTERM 安全停止，不做 SIGKILL、sudo、批量操作或任意 PID 文本输入。
前端先显示确认弹窗：进程名、PID、所选端口和同 PID 其他已知端口；默认聚焦“取消”，提交后禁用重复点击。
Rust Command terminate_listener({request:{pid,processStartTime,port}}) 必须重新读取进程身份与监听端口；若 PID 不存在、进程启动时间不同、端口已不存在、不能验证身份、PID 是 DevDock 本身或无权限则拒绝。startTime 缺失时禁用 UI 操作。调用 sysinfo::kill_with(Signal::Term)，不能使用会发送 Kill 的 kill()。
发送后最多等待 2 秒确认原进程身份是否消失；未退出时返回 signalSent=true/processExited=false，不展示为已停止。无权限/进程变化/旧 PID 均不得误操作。
编写验证失败路径单元测试。真实 Mac 上只对当前任务自己启动的 python3 测试服务验证发送 SIGTERM。禁止测试其他正在工作的服务。
```

**验收：** 正确终止临时服务；故意传错 startTime、port 均被拒绝；自进程拒绝；任何调用都不会提权；无已验证退出时不声明“已停止”。

---

## P5｜回归、构建、DMG 安装验证

**前置：** P4。 **产出：** 交付 `.app`/`.dmg`、可追溯验证记录。

**Codex 指令：**

```text
执行 DevDock v0.1 Release Gate（docs/06_测试与发布.md）。验证 pnpm build、cargo test、cargo check，执行 lsof/parser、收藏、URL 验证、健康状态、SIGTERM 负路径测试。
检查项目不包含 Mock 数据或无效按钮；Tauri 仅包含必要权限；无 sudo、Shell 注入、机密泄露。固定图标、版本 0.1.0、bundle identifier top.bugstack.devdock；检查 GUI 环境中 /usr/sbin/lsof 可用。
在 macOS 执行 pnpm tauri build --bundles dmg；找到实际 .app 和 .dmg，使用 Finder 安装 .app 到 Applications，再验证真实扫描、收藏、健康检查和安全停止临时服务。
补齐最少的 README 安装和使用说明。报告执行过的命令、退出码、手动测试项、真实产物路径、签名与公证状态，以及未验证内容。非 macOS 不可宣称 DMG 构建已通过。
不要追加菜单栏、Docker、自动更新或其他 v0.2 功能。
```

**验收：** 所有高优先级测试通过；DMG 安装版 Mac 端真实功能通过；列出当前仍存在的 macOS 限制与分发风险。

---

## 汇报模板（每阶段）

```text
阶段：P?
完成：
- [具体功能]
变更文件：
- path/to/file
验证：
- command — PASS / FAIL / NOT RUN（说明原因）
- 手工场景 — PASS / FAIL / NOT RUN
剩余限制：
- [明确未验证/未完成]
下一阶段：P?
```

每阶段达到要求后停止；不得为耗尽 Token 故意扩大范围。
