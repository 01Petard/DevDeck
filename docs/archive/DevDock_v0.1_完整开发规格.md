> 历史规格快照：2026-10-23 的初始交付包，保留原文供追溯。这里的状态、命令和未实现描述不代表当前项目。维护中的规格与开发入口见 [文档导航](../00_开始开发.md)。

# DevDock v0.1 — 完整开发交付规格（单文件版）

本文件汇集整个 `docs/` 文档包，便于一次性上传到 ChatGPT Work 或其他开发 Agent。由同一份分章节文档自动合并而成。



---

# DevDock v0.1 — 开发入口

> 版本：0.1.0（目标）｜文档发布日期：2026-10-23｜目标平台：macOS｜状态：**开发规格，非已实现软件**

## 目标

做一个安装后可直接使用的 macOS 桌面工具，用于：**发现本机 TCP 监听端口 → 查看进程 → 收藏常用端口 → 对回环地址执行 HTTP 健康检查 → 安全终止用户确认的进程**。

产品不是 Docker 管理器、应用启动器或多机运维平台。首版追求：真实结果、低误报、安全操作、可打包 `.dmg`。

## 文档导航

| 文件 | 核心问题 | 读者 |
|---|---|---|
| `01_产品需求_PRD.md` | 做什么、什么不做、业务规则与完成定义 | 产品/前端/开发 |
| `02_技术设计.md` | Tauri、macOS、lsof、sysinfo、HTTP、安全停止等如何实现 | Rust/架构 |
| `03_数据契约与错误码.md` | Rust↔Vue 数据结构、IPC、校验、持久化 | 前后端 |
| `04_交互与界面.md` | 单窗口组件、行为、空状态、危险操作确认 | Vue |
| `05_Codex_分阶段任务.md` | P0–P5 可直接复制执行的实施任务与阶段验收 | Codex/Work |
| `06_测试与发布.md` | 自动化、人工验收、DMG 安装测试与发布门槛 | QA/交付 |
| `07_决策与风险.md` | 已冻结决策、未知事项、风险边界 | 所有人 |
| `../AGENTS.md` | 给 Codex/Work 的仓库级持续指令 | AI 编码工具 |

## 执行指引（推荐）

1. 建立空的 `DevDock` 仓库；将此交付包内的 `docs/` 与 `AGENTS.md` 复制到仓库根目录。
2. 在 macOS 环境启动 Codex 或 ChatGPT Work；让它先读取 `AGENTS.md` 与本文件。
3. 逐个执行 `05_Codex_分阶段任务.md` 的 P0→P5。**完成当前阶段的验收后，再进行下一阶段**。
4. 所有真实的进程终止测试，仅对开发任务中启动的临时测试服务进行。不得用生产/正在工作的重要进程做测试。
5. 最终以 `06_测试与发布.md` 中的 Release Gate 作为 v0.1 完成依据。

## 非目标

不引入 Spring Boot、数据库、Redis、Node 常驻服务、账号、云同步、Docker 管理、日志采集、窗口托盘、远程监控、进程重启、Shell 终端、强制杀进程、自动升级或 CI/CD 发布流水线。

## 环境基线

- macOS，需能安装并运行本地 GUI 应用；优先在 Apple Silicon 真机验证。
- Node.js：使用当前受支持的 LTS 版本；包管理器 `pnpm`。
- Rust stable，**Store 插件最新文档要求 Rust ≥ 1.90**，以安装时官方要求和实际 lockfile 为准。
- Xcode Command Line Tools：`xcode-select -p` 应成功。
- 版本通过官方 Tauri 2 Vue/TypeScript 初始化程序选择当前兼容稳定版本并锁定 lockfile；不要自行捏造版本。

## 快速启动命令（在项目初始化完成后）

```bash
pnpm install
pnpm tauri dev
pnpm build
(cd src-tauri && cargo test && cargo check)
pnpm tauri build --bundles dmg
```

`tauri build --bundles dmg` 必须在 **Mac** 执行；若开发环境不是 macOS，标记为“未验证”，不得写成已通过。

## 权威参考（核对于 2026-10-23）

- Tauri 2 项目初始化：https://v2.tauri.app/start/create-project/
- Tauri Store：https://v2.tauri.app/plugin/store/
- macOS DMG：https://v2.tauri.app/distribute/dmg/
- macOS 签名：https://v2.tauri.app/distribute/sign/macos/
- sysinfo Process：https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html
- reqwest ClientBuilder：https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html
- lsof field output：https://lsof.readthedocs.io/en/stable/manpage/


---

# 01｜DevDock 产品需求（PRD）

## 1. 产品定位

**一句话：** 开发者在 macOS 上排查“哪个进程占了端口、服务到底好没好、能否安全停掉它”的桌面工具。

核心使用场景：

- Vite 启动报端口占用：快速找出监听端口和 PID。
- Spring Boot 服务启动后：确认端口处于 TCP LISTEN、再检查 `/actuator/health` 返回 2xx。
- 日常开发：收藏 8080/5173/5432 等端口；本次服务不在运行时仍显示收藏。
- 测试进程需要退出：对**自己有权限终止**的进程进行二次确认，优先优雅退出。

目标用户：日常使用 Java/Spring、Node/Vite、PostgreSQL、Redis、Docker Desktop 等本地开发工具的 macOS 开发者。**不承诺识别应用框架或项目名称**，只显示操作系统真实进程信息。

## 2. 功能需求及可验证行为

| ID | 功能 | MVP 验收 |
|---|---|---|
| FR-01 | TCP 端口扫描 | 启动时扫描，发现权限范围内的 `LISTEN`，包含 PID/进程名/地址/端口 |
| FR-02 | 手动刷新 | 刷新后可发现新增监听及已关闭服务 |
| FR-03 | 搜索 | 支持按端口、PID、进程名（不区分大小写）过滤 |
| FR-04 | 进程详情 | 显示真实 PID、可获取的路径/启动时间；不可获取显示“不可用” |
| FR-05 | 收藏端口 | 对端口添加别名，支持空闲端口，重启应用后保留 |
| FR-06 | HTTP 健康探测 | 仅用户显式配置的本机回环 HTTP(S) URL；显示 2xx/其他/连接失败/超时和耗时 |
| FR-07 | 安全终止 | 明确二次确认；验证 PID+启动时间+监听端口；仅 SIGTERM；不提权 |
| FR-08 | DMG | 在 Mac 上生成 `.dmg`，安装后不依赖开发环境即可运行 |

## 3. 不在本期范围

- 不会“启动/重启/守护”进程，不处理 shell 脚本和用户命令。
- 不采集日志、CPU/内存历史或多次健康历史。
- 不提供数据库、账户登录、API Server、远程调用或云同步。
- 不识别 Docker 容器所属、Java 项目名、Spring Boot 应用名或 Vite 工作目录。
- 不做 UDP 监听发现、不扫描局域网、不开放任意域名 HTTP 检测。
- 不做菜单栏驻留、开机启动、自动更新、代码签名/公证流水线。

## 4. 行为规则

### 4.1 监听端口

- 列表项代表**一个监听 Socket 记录**，不是一个“服务”。
- 同一 PID 可有多个 Socket，同一端口可能有多个地址/IPv4/IPv6 记录。
- 每次扫描是快照；扫描的瞬间进程可能退出，须适当降级。
- 监听代表 TCP 端口存在，**不代表 HTTP 健康**。
- 扫描遇到权限限制，允许展示可见子集；不得把不可见进程当成“不存在”。
- 不执行 `sudo`，不弹窗索要管理员密码。

### 4.2 收藏

- 收藏项 Key 为 `port`，合法范围 1..65535；收藏同端口时更新现有项。
- 可为一个未监听端口建立收藏。
- 别名允许空白，去除首尾空格后最大 60 字符。
- `healthUrl` 可为空；为空时只显示 TCP 状态；**不自动尝试 HTTP**。
- 收藏地址的端口与收藏端口推荐一致；MVP **要求 URL 的显式/默认有效端口等于收藏端口**，避免展示错误归属。特殊部署场景留给后续版本。
- 一个收藏可能对应多个监听 Socket。UI 显示多个监听候选，用户必须选中具体进程才能停止。

### 4.3 HTTP 健康检测

- 仅 `http`/`https`；host 仅 `localhost`、`127.0.0.1`、`::1`。不接受含用户名/密码的 URL。
- 不允许 `localhost.evil.com` 等名称，不支持其他 127/8 地址或 link-local 地址（首版明确收窄）。
- `localhost` 连接必须锁定为系统本机回环地址，不能依赖外部 DNS/代理解析；禁用 HTTP 代理和重定向。
- 固定 GET，超时 3 秒；只判断响应头及状态码，不下载正文。
- 2xx `healthy`，3xx/4xx/5xx `http_error`；连接/TLS 失败 `network_error`；超时 `timeout`。
- `healthy` 不等同于业务健康；它仅意味着配置的 URL 返回 HTTP 2xx。
- 收藏项可手动触发探测；点击“刷新”时可以重新检测已有收藏，限制并发至 4。

### 4.4 结束进程

- 必须先在 UI 选择一个**真实监听记录**，不能仅输入 PID 来结束任意进程。
- 必须展示 PID、进程名、请求操作的端口与“可能影响同 PID 其他端口”的警告。
- `processStartTime` 必须已获取且大于 0；否则终止按钮禁用。
- Rust 端必须重新检查进程身份和端口监听；PID 已被替换/端口变化则拒绝。
- 拒绝结束 DevDock 自身进程、无权限的进程、明显的内核特殊 PID；不使用 `sudo`/`SIGKILL`。
- SIGTERM 成功发送不等于进程退出；允许返回“信号已发送，尚未退出”。
- 已退出/不存在时报告当前结果并刷新；不重试到新的 PID。

## 5. 完成定义（DoD）

1. 含可运行的 Tauri 2 原生应用、真实 macOS 采集和持久化配置。
2. 自动测试覆盖关键解析器、校验逻辑和危险操作失败场景。
3. 真实 Mac 验证一个临时 HTTP 服务从出现、健康、到停止的完整流程。
4. 在 Mac 生成 DMG，使用 Finder 安装后验证扫描/收藏/健康/停止功能。
5. 清楚记录未验证项目、平台限制与签名/公证状态。

## 6. 有意接受的限制

`lsof` 受到 macOS 权限与进程快速变化影响；PID+启动时间二次校验可以降低误操作，但不是原子性进程句柄保证。MVP 不承诺绝对零竞态，不进行提权或底层内核 API 开发。


---

# 02｜技术架构与核心实现

## 1. 技术选择

- **Desktop**：Tauri 2（macOS 唯一目标平台），官方 Vue 3 + TypeScript + Vite 模板，pnpm。
- **UI**：Vue 响应式状态 + 原生 CSS + 可选轻量图标库；不引入 Pinia、Vue Router、组件大框架。
- **系统发现**：`lsof` 的 `-F` 机器可解析输出，Rust 子进程执行；`sysinfo` 补充进程信息。
- **健康探测**：Rust `reqwest` 的异步 Client，禁用代理/重定向，3 秒总超时。
- **终止**：`sysinfo::Process::kill_with(Signal::Term)`；不得使用默认 `kill()`（该方法发送 SIGKILL）。
- **收藏持久化**：Tauri Store 插件；不用数据库。
- **IPC**：3 个 Tauri Commands；系统能力不经 HTTP 暴露给浏览器。

设计原则：主流程清晰，独立职责拆在 `ports.rs`、`health.rs`、`processes.rs`，避免 Manager/Provider 等无意义包装。

## 2. 数据流

```text
Vue App
  ├─ invoke('list_listeners') ───────> ports.rs -> lsof -> parse -> sysinfo -> Vec<PortListener>
  ├─ invoke('check_http_health') ────> health.rs -> validate URL -> reqwest -> HealthCheckResult
  ├─ invoke('terminate_listener') ───> processes.rs -> revalidate -> SIGTERM -> bounded poll
  └─ Store (plugin) ────────────────> devdock.json (favorites only)
```

### 2.1 项目目录（最小推荐）

```text
DevDock/
├── AGENTS.md
├── docs/...
├── src/
│   ├── main.ts
│   ├── App.vue
│   ├── style.css
│   ├── components/
│   │   ├── ListenerList.vue
│   │   ├── ListenerDetail.vue
│   │   ├── FavoriteEditor.vue
│   │   └── TerminateDialog.vue
│   ├── services/
│   │   ├── native.ts
│   │   └── settings.ts
│   └── types/devdock.ts
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands.rs
│   │   ├── ports.rs
│   │   ├── processes.rs
│   │   └── health.rs
│   ├── capabilities/default.json
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

以官方模板为起点，不要为了与示例完全一样而移动其必要文件。

## 3. 端口发现：lsof

执行 macOS 系统工具，不能通过 Shell：

```bash
/usr/sbin/lsof -nP +c 0 -iTCP -sTCP:LISTEN -F0pcftn
```

`-F0` 是以 NUL 分隔字段的输出形式；`p` PID、`c` 命令、`f` 文件描述符、`t` 文件类型（IPv4/IPv6）、`n` 名称。lsof 会按进程/文件记录组合输出，需按真实输出格式解析，而不是假设每行只有单个端口。`-F0` 的进程/文件记录换行边界以 macOS 实测 fixture 为准。

需防护：

- 不调用 `sh -c`，只传固定参数给固定系统命令路径。
- 命令执行超时 5 秒；超时终止 `lsof` 子进程（不影响被检查的其他进程）。
- stdout 最大 8 MiB，超过认为异常，避免无上限内存消耗；stderr 仅保留合理长度用于错误代码（不要记录敏感信息）。
- `lsof` 返回“无匹配项”时可能是非 0 退出码；需依据 stdout/stderr/退出码识别，不要把权限或执行故障静默转为空列表。
- 解析 `n` 从**右侧**最后的冒号读取端口，正确识别 `*:8080`、`127.0.0.1:8080`、`[::1]:8080`。对不符合预期的 Socket 名称跳过并记录可观测的解析计数。
- 使用 `t` 判断 IPv4/IPv6，不能只依赖地址文本推测。
- 记录去重 Key：`(pid, localAddress, port, ipVersion, protocol)`；按端口→PID→地址排序。
- 将从 `lsof` 获得的 PID 批量交给一次 `sysinfo` 刷新补充进程名/启动时间/路径，避免每行重复刷新进程列表。
- 原始 lsof 进程名可以作为 `sysinfo` 不可用时的退化展示值；但 **startTime 不允许推测**。

`sysinfo` 的启动时间按 UNIX 秒记录；不可用或 0 时转为 null。对于非 UTF-8 路径采取有损 UTF-8 显示，不作为身份校验依据。

## 4. HTTP 探测

建议通过单一 `reqwest::Client` 复用连接；关键设置：

```rust
reqwest::Client::builder()
    .no_proxy()
    .redirect(reqwest::redirect::Policy::none())
    .timeout(std::time::Duration::from_secs(3))
    .build()?;
```

这是关键配置片段，非完整可编译模块。开发时核对锁定的 `reqwest` 版本 API。

URL 处理：

1. 使用 URL 解析器，不使用字符串前缀 `starts_with("http://localhost")`。
2. 限制协议、精确 host、拒绝 userinfo、拒绝 fragment；端口合法且与收藏端口一致（收藏保存时）。
3. 对 `localhost` 的连接确保只会抵达 `127.0.0.1` 或 `[::1]`，必要时使用 reqwest 的地址覆盖能力或等效安全机制；不得通过任意 DNS 解析转发到非回环地址。
4. 禁用代理和重定向。不携带授权头、Cookie；响应只读取状态码，避免下载未知大小内容。
5. 耗时从请求开始计到收到响应头，网络错误合理脱敏；不把完整 URL 中可能含敏感 query 的内容写到日志中。
6. 并发最多 4 个；同一收藏一次只允许一个进行中的请求。

**注意：** GET 可能改变设计不规范的本地服务状态。仅对用户明确配置的地址发送请求，不进行自动探测。

## 5. 安全终止

严格从 UI 实际监听项发起，且用户进行确认：

```text
选中 Listener
 → 显示确认（name / PID / port / 所有已知同 PID 端口）
 → invoke terminate_listener({request})
 → Rust 重新读该 PID 的 startTime、当前监听端口
 → 比对请求；检查不是 DevDock 自身、PID 非特殊值
 → sysinfo::kill_with(Signal::Term)
 → 最多 2 秒轮询该进程身份是否仍存在（不能调用无限等待）
 → 返回 signalSent/processExited，Vue 刷新端口数据
```

Rust 端绝不能信任“前端已确认”来跳过校验。信号发送后只检查**原进程身份**是否仍存活，不应因为 PID 被复用而误报告旧进程仍运行。

**剩余风险：** macOS 不提供本设计中原子性进程句柄。PID 与秒级启动时间不能彻底消除 TOCTOU 风险；采取先核验、立即发送 SIGTERM、绝不重试到新进程的保守实现。如无法可靠判定状态，应返回“不确定/已发送”而不是“已结束”。

## 6. 权限与安全

- Tauri 默认 Core 权限与 Store 所需权限，在 `capabilities/default.json` 中按需配置；不要开启 Shell 插件或广泛文件访问。
- Store 官方文档示例使用 `store:default`，具体功能所需权限和实际版本确认一致。
- 不实现 Node 本地 HTTP Server；WebView 调用的自定义 Rust Command 属于应用内部边界，不对外暴露。
- 不输出环境变量、完整命令行、认证凭据或包含敏感查询参数的健康检测 URL 到日志。
- 不提权；进程管理受当前用户账户权限约束。

## 7. 性能边界

- 启动和刷新扫描，首版不常驻高频轮询；窗口 focus 恢复时可节流（例如 2 秒内只执行一次）。
- 若触发重复刷新，以单飞（single-flight）模式共享或拒绝新请求，避免 lsof 堆叠。
- 数据通常数十/数百条，没必要引入虚拟滚动或缓存层。
- 展示最新扫描时间；扫描结果过时不允许继续对旧进程执行危险操作，后端仍须二次验证。

## 8. 可靠性约束

- 如果 `lsof` 执行失败，界面显示错误而不是“0 个端口”。
- 如果存储配置损坏，展示错误与恢复选项，不静默覆盖用户原始数据。
- Store 保存失败后保留编辑状态并提示，不声称“已保存”。
- 临时进程退出可导致部分记录缺少详情；不能因此令整个端口列表失败。

## 9. 参考

- https://lsof.readthedocs.io/en/stable/manpage/
- https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html
- https://docs.rs/sysinfo/latest/sysinfo/struct.System.html
- https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html
- https://v2.tauri.app/plugin/store/


---

# 03｜数据契约、命令接口、校验与错误码

所有 Rust↔Vue JSON 对象采用 **camelCase**。Rust 的 serde 结构体可以通过 `#[serde(rename_all = "camelCase")]` 实现。TypeScript 类型存放在 `src/types/devdock.ts`；若引入类型生成机制会增加复杂度，MVP 允许手动保持一致并用契约测试验证。

## 1. TypeScript 类型

```ts
export type IpVersion = 4 | 6 | null;

export interface PortListener {
  pid: number;                     // >0
  processName: string;             // 原始进程名或可用的 lsof 退化名称
  processStartTime: number | null; // Unix seconds；不可获取时 null
  executablePath: string | null;
  protocol: 'TCP';
  address: string;                 // *, 127.0.0.1, ::1 等
  port: number;                    // 1..65535
  ipVersion: IpVersion;
}

export interface FavoritePort {
  port: number;                    // 收藏的唯一键
  label: string;                   // trim 后 ≤60
  healthUrl: string | null;        // 仅回环 HTTP(S) URL，端口需等于 port
}

export interface DevDockSettings {
  schemaVersion: 1;
  favorites: FavoritePort[];
}

export type HealthStatus = 'healthy' | 'http_error' | 'network_error' | 'timeout';

export interface HealthCheckResult {
  url: string;
  status: HealthStatus;
  statusCode: number | null;
  latencyMs: number | null;
  checkedAt: number;               // Unix milliseconds
  message: string | null;
}

export interface TerminateRequest {
  pid: number;
  processStartTime: number;        // 必须 >0，不能省略或填 null
  port: number;
}

export interface TerminateResult {
  signalSent: boolean;            // SIGTERM 是否发送成功
  processExited: boolean;         // 2 秒内确认原进程已消失
  message: string;
}

export type CommandErrorCode =
  | 'INVALID_ARGUMENT'
  | 'SCAN_FAILED'
  | 'SCAN_TIMEOUT'
  | 'PROCESS_NOT_FOUND'
  | 'PROCESS_IDENTITY_CHANGED'
  | 'PROCESS_UNVERIFIABLE'
  | 'PROCESS_PROTECTED'
  | 'PERMISSION_DENIED'
  | 'SIGNAL_FAILED'
  | 'HTTP_URL_NOT_ALLOWED'
  | 'INTERNAL_ERROR';

export interface CommandError {
  code: CommandErrorCode;
  message: string;                 // 不应包含隐私数据和底层敏感细节
}
```

建议 Tauri 命令失败返回可序列化结构化错误对象，而不是拼接一个长字符串。Rust 示例：`Result<T, CommandError>`，`CommandError` 实现 `Serialize`。具体 `invoke` 错误值通过适配函数做显式规范化。

## 2. IPC Commands（仅三个）

| IPC 名 | 输入 | 成功输出 | 主要错误 |
|---|---|---|---|
| `list_listeners` | 无 | `PortListener[]` | `SCAN_FAILED`, `SCAN_TIMEOUT` |
| `check_http_health` | `{ url: string }` | `HealthCheckResult` | `HTTP_URL_NOT_ALLOWED`, `INVALID_ARGUMENT` |
| `terminate_listener` | `{ request: TerminateRequest }` | `TerminateResult` | `PROCESS_*`, `PERMISSION_DENIED`, `SIGNAL_FAILED` |

前端调用：

```ts
import { invoke } from '@tauri-apps/api/core';

const listeners = await invoke<PortListener[]>('list_listeners');
const health = await invoke<HealthCheckResult>('check_http_health', {
  url: 'http://127.0.0.1:8080/actuator/health',
});
const terminated = await invoke<TerminateResult>('terminate_listener', {
  request: { pid: 12345, processStartTime: 1792756200, port: 8080 },
});
```

以上为**项目期望实现的自定义 IPC 接口**，不是 Tauri 已内置的命令。

## 3. 示例扫描数据

```json
[
  {
    "pid": 18243,
    "processName": "java",
    "processStartTime": 1792756200,
    "executablePath": "/Library/Java/JavaVirtualMachines/jdk-17.jdk/Contents/Home/bin/java",
    "protocol": "TCP",
    "address": "127.0.0.1",
    "port": 8080,
    "ipVersion": 4
  },
  {
    "pid": 23910,
    "processName": "node",
    "processStartTime": null,
    "executablePath": null,
    "protocol": "TCP",
    "address": "::1",
    "port": 5173,
    "ipVersion": 6
  }
]
```

示例值用于**契约说明**，正式 UI 必须使用真实系统数据。

## 4. 收藏配置存储

Store 文件名：`devdock.json`，逻辑根键：`settings`。配置：

```json
{
  "schemaVersion": 1,
  "favorites": [
    {"port": 8080, "label": "Spring Boot API", "healthUrl": "http://127.0.0.1:8080/actuator/health"},
    {"port": 5432, "label": "PostgreSQL", "healthUrl": null}
  ]
}
```

Store 使用推荐方式：

```ts
import { load } from '@tauri-apps/plugin-store';
const store = await load('devdock.json', { autoSave: false });
await store.set('settings', settings);
await store.save();
```

启动时读取缺失配置初始化 `{schemaVersion: 1, favorites: []}`；如果旧数据损坏、版本高于 1 或字段类型不符，**不可直接静默覆盖**，应让用户知道发生了配置问题。

只持久化收藏，**不保存 PID、扫描记录、健康历史、进程退出操作记录**。

## 5. 输入规则

- `port`：整数 1..65535。
- `pid`：正整数，不能是 DevDock PID，拒绝系统特殊 PID。
- `processStartTime`：正整数，单位为 Unix 秒，不接受 null、0、NaN 或客户端传入的字符串。
- `label`：去首尾空格后 ≤60 个 Unicode 字符。
- `healthUrl`：可为 null；非空须为合法 HTTP(S) URL、指定主机精确匹配 `localhost`/`127.0.0.1`/`::1`、拒绝 username/password、查询参数慎用，长度 ≤2048 字符；收藏时有效端口必须等于 port。
- `check_http_health` 命令也必须在 Rust 层独立校验 URL，不信任前端。

URL 对回环主机的安全连接约束参见 `02_技术设计.md`。不要使用前缀字符串检查。

## 6. 异常契约

- `list_listeners` 失败：reject `CommandError`，前端保留已有快照但显著标记**扫描失败且数据已过期**，不能伪装成最新正常结果。
- `check_http_health` 的 4xx、5xx、超时、连接失败：属于正常诊断结果，**resolve `HealthCheckResult`**；非法 URL 是 reject 错误。
- `terminate_listener` 的 PID/端口/时间校验失败：reject `CommandError`，**不得发送任何信号**。
- SIGTERM 成功而短时间未退出：resolve `{signalSent:true,processExited:false,...}`，不能显示“已成功停止”。
- IPC 调用失败时前端不得向用户泄露完整命令行、环境变量或多余敏感路径。

## 7. 状态映射

| 维度 | 状态 | 含义 |
|---|---|---|
| 端口 | listening / not_listening / scan_unknown | 监听 / 未监听 / 扫描失败无法判断 |
| 健康 | unconfigured / checking / healthy / http_error / network_error / timeout | 无配置 / 检测中 / 其余详见数据契约 |
| 终止 | idle / confirming / sending / signal_sent / exited / failed | 用户动作状态，不是进程生命周期状态 |

避免把“端口不存在”、“HTTP 返回 500”、“扫描失败”合并为同一个红色失败状态。


---

# 04｜UI 与用户交互规格

## 1. 页面与布局

MVP 只有**一个主窗口**（建议初始 1000×680；最小 780×520），不要路由系统。布局：

```text
┌────────────────────── DevDock ──────────────────────────┐
│ 搜索端口/PID/进程                    刷新    上次扫描时间 │
├──────────────────────────┬─────────────────────────────┤
│ 收藏端口                 │ 选中监听详情                │
│ ★ 8080 Spring Boot       │ 端口 8080 / TCP LISTEN     │
│ ★ 5173 Vite              │ PID 12345  名称 java       │
│ ★ 5432 PostgreSQL 未监听 │ 地址 127.0.0.1            │
│                          │ 启动时间/可执行路径         │
│ 其他监听                 │                             │
│ 3000 node                │ HTTP 健康 URL（仅收藏）     │
│ 6380 redis-server        │ [立即检测] HTTP 200 / 22ms │
│                          │                             │
│                          │ [收藏/编辑]   [结束进程]    │
└──────────────────────────┴─────────────────────────────┘
```

正常字号、等宽端口及 PID、跟随 macOS 亮暗主题。无需展示项目图标、图表、大数据屏幕或复杂动效。

## 2. 页面元素

- 顶部：输入式搜索、刷新按钮、最近一次成功扫描时间、错误提示。
- 左侧“收藏”：即使端口未监听仍显示；如果同一端口有多个监听 Socket，允许展开候选记录。
- 左侧“其他监听”：只展示没有被收藏覆盖的监听记录；按照端口→PID排序。
- 右侧：监听状态、PID、进程名、启动时间、路径、地址、操作。
- 右侧“健康”：仅存在对应收藏并已设置 URL 时出现可点击“检测”；否则提示“未配置”。
- 右侧“结束”：禁用条件要有 Tooltip 或说明，不是无声禁用。

## 3. 关键交互

### 3.1 启动/刷新

- 启动自动扫描一次，扫描期间展示 loading（保留上次结果可选，但要标旧）。
- 手动刷新重新扫描，防重复触发；焦点恢复自动刷新需节流。
- 端口已消失时收藏保持；如果失去所选 `pid+startTime+socketKey`，详情换为空状态，不保留危险操作对象。
- 扫描出错应显示“扫描失败，结果未更新”，而不是“没有服务”。

### 3.2 收藏

- 星号收藏当前端口（默认 label 空，healthUrl null）。
- 也提供“添加端口”输入，以便提前收藏还没有启动的端口。
- 在同一小型编辑面板中编辑 alias、healthUrl，保存时校验全部字段。
- Store 保存成功后再反馈“已保存”；失败则保留编辑值并提示重试。

### 3.3 健康检测

- 用户触发时显示 loading；同一 URL 不允许叠加请求。
- `200–299` 绿色“HTTP 200 · 22ms”；`301/404/500` 显示对应码并说明非 2xx；连接失败/超时各有文本反馈。
- `localhost` URL 不保证对应外部监听地址；收藏键与健康 URL 端口须一致。
- HTTP 健康检查只显示当前结果，关闭应用不恢复旧检测状态。

### 3.4 危险操作确认

弹窗标题：**结束进程？**

文案示例：

> 将向 `java`（PID 18243）发送 SIGTERM。该进程可能同时监听 8080、8081 等端口。终止可能中断正在进行的本地任务。

按钮：**取消**（默认聚焦）、**发送 SIGTERM**（危险强调）。

- 进程启动时间缺失：隐藏或禁用动作，提示“无法验证进程身份，不能安全终止”。
- 发送成功但未退出：提示“已发送 SIGTERM，进程尚未退出”，不要自动升级为强制终止。
- 进程已经变化：拒绝、刷新，要求用户重新选择。

## 4. UX 验收

1. 不打开终端即可定位占用 8080 的进程。
2. 常用端口未运行时仍能在收藏区看到。
3. 用户不会把 TCP 监听误认为 HTTP 2xx。
4. 一次刷新失败不会让用户误以为全机端口都消失了。
5. 结束按钮不可能一键触发；成功与未退出状态明确区分。
6. 深浅主题、空状态、错误信息都有可读的文本表达，不单靠图标颜色。


---

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


---

# 06｜测试、验收与 DMG 发布

## 1. 自动化测试范围

### Rust

- lsof `-F` 解析器：`p/c/f/t/n` 字段分组、NUL 分隔与记录边界、IPv4、IPv6、通配、重复行、异常 port、退出时缺失进程、无匹配输出。
- 进程身份比对：PID 错误/启动时间不同/端口消失/本人进程/不可确认时拒绝。
- URL 校验：精确 localhost/127.0.0.1/[::1] 允许；localhost.evil.com、其它 IP、ftp、带 userinfo 等拒绝。
- HTTP 状态：200/204、301、404/500、拒绝连接、超时（本地可控测试服务器）。
- **所有涉及 SIGTERM 的自动测试只能操作测试创建的子进程**；禁止随便找一个 PID 发信号。

### Vue/TypeScript

- 收藏项校验、重名同端口更新、Store 读写失败反馈。
- 搜索过滤、服务列表去重显示、旧选中进程失效处理。
- 健康与监听状态独立展示；SIGTERM 未退出不可显示为已停止。
- 简单逻辑可以用单元测试，避免为了测试 UI 框架内部实现而过度 mocking。

## 2. 手动验收矩阵（Release Gate）

| 编号 | 场景 | 可验证结果 | 优先级 |
|---|---|---|---|
| T01 | 启动临时服务 18765 | 识别端口与真实 PID | 阻断 |
| T02 | 服务停止后刷新 | 监听记录消失，旧目标不可操作 | 阻断 |
| T03 | 搜索端口/PID/进程名 | 过滤正确且大小写无关 | 必须 |
| T04 | 同 PID 多监听端口 | 不把进程误当单一端口 | 阻断 |
| T05 | IPv4/IPv6 Socket | 显示地址与版本，不错误去重 | 必须 |
| T06 | 收藏未监听端口 | 持续显示“未监听” | 必须 |
| T07 | 重启后恢复收藏 | 别名、healthUrl 完整一致 | 阻断 |
| T08 | HTTP 200/204 | `healthy`、状态码、耗时 | 阻断 |
| T09 | HTTP 404/500 | `http_error`，不是网络错误 | 必须 |
| T10 | 连接失败/3s 超时 | 正确区分并可再次测试 | 必须 |
| T11 | HTTP 301/302 | 不跟随，显示非健康 | 必须 |
| T12 | 非回环 URL | Rust 命令层拒绝 | 阻断 |
| T13 | 终止确认弹窗 | 提示 PID、端口、同 PID 影响 | 阻断 |
| T14 | 错误 startTime 或 port | 无信号发送，返回身份变化 | 阻断 |
| T15 | 尝试操作 DevDock 自身 | 拒绝 | 阻断 |
| T16 | SIGTERM 临时 Python 服务 | 发送成功后正确确认或报告未退出 | 阻断 |
| T17 | 无权限进程 | 不提权，安全失败 | 必须 |
| T18 | GUI 环境非终端启动 | 扫描仍工作（固定 lsof 路径） | 阻断 |
| T19 | DMG 安装后的应用运行 | 不依赖 Node/Rust/开发终端 | 阻断 |
| T20 | 配置损坏/保存失败 | 可见错误，不静默清空 | 必须 |

## 3. 测试命令

```bash
# 工程内
pnpm install
pnpm build
(cd src-tauri && cargo test)
(cd src-tauri && cargo check)

# macOS 临时服务，单独终端运行
python3 -m http.server 18765 --bind 127.0.0.1

# macOS 系统对照
/usr/sbin/lsof -nP -iTCP:18765 -sTCP:LISTEN

# macOS 打包
pnpm tauri build --bundles dmg
```

健康异常页建议使用 Rust/Python 的受控 localhost 测试服务器自行返回 200/204/301/404/500 与延迟。不必增加正式应用 HTTP 服务。

## 4. DMG 操作步骤

1. 确认在 Mac 上、Rust 与 Xcode CLT 齐全。
2. 通过 `pnpm build`、`cargo test`、`cargo check`。
3. 配置应用名 `DevDock`、版本 `0.1.0`、Bundle ID `top.bugstack.devdock`、图标。
4. 执行 `pnpm tauri build --bundles dmg`。
5. 根据 CLI 输出找到实际 DMG；通常在 `src-tauri/target/release/bundle/dmg/`，**以实际产物为准**。
6. 双击 DMG、将 App 拖入 `Applications`，**从 Finder 启动安装版**，不可只验证 `tauri dev`。
7. 使用安装版重复 T01、T07、T08、T12、T16、T18。
8. 记录二进制实际架构（arm64/x86_64）、OS 版本、是否签名、公证与系统安全提示。

## 5. 签名与分发说明

生成 DMG ≠ 公证/签名完成。首版**本地自用**可先完成打包和安装验证。对外分发需研究 Developer ID 签名、Apple notarization、Gatekeeper 行为。不要指导用户不加判断地永久关闭 Gatekeeper。

官方参考：
- https://v2.tauri.app/distribute/dmg/
- https://v2.tauri.app/distribute/sign/macos/

## 6. 发布判断

- **PASS**：所有“阻断”用例在真机与安装版通过；其余失败已明确评估且不影响 v0.1 核心目标。
- **BLOCKED**：无法构建 Mac DMG、存在误停止进程风险、URL 非回环校验可绕过、错误将扫描失败解释为 0 端口。
- **NOT VERIFIED**：没有实际 Mac 或无法执行的用例要清楚标记。不能把代码审查或理论可行当成真机验证。

## 7. 交付报告模板

```text
DevDock v0.1 Release Report
Target OS / architecture:
Commit SHA:

Commands:
- pnpm build: PASS/FAIL
- cargo test: PASS/FAIL
- cargo check: PASS/FAIL
- pnpm tauri build --bundles dmg: PASS/FAIL/NOT RUN

Manual acceptance:
- T01..T20: PASS/FAIL/NOT RUN

Artifacts:
- .app: <实际路径>
- .dmg: <实际路径>

Signing/notarization: 未签名 / ad-hoc / Developer ID + 公证（按事实）
Known limitations:
- ...
```


---

# 07｜冻结的技术决策、已知风险和未决验证

## 1. 已冻结的决策

| 编号 | 决策 | 原因 |
|---|---|---|
| ADR-01 | 只做 macOS | 降低兼容性负担，达成首个 DMG |
| ADR-02 | Tauri 2 + Vue 3 + Rust | Web UI 熟悉，原生系统能力放在 Rust |
| ADR-03 | `lsof` 扫描而非内核私有接口 | 开发量可控，原生系统工具现成 |
| ADR-04 | `sysinfo` 补充进程元数据与 SIGTERM | 简化进程查询和信号发送 |
| ADR-05 | 仅 TCP LISTEN | UDP/连接流量并非 MVP |
| ADR-06 | 健康 URL 仅回环 GET | 功能够用，避免任意网络代理能力 |
| ADR-07 | Tauri Store、不引入 SQLite | 收藏数据量极小 |
| ADR-08 | 终止必须校验进程身份，不允许 SIGKILL | 降低误操作风险 |
| ADR-09 | 没有自动监控后台守护 | 降低资源消耗与复杂度 |
| ADR-10 | 不采用前端 Mock 作为正式功能 | 必须直接验证操作系统 |

## 2. 关键风险与对策

| 风险 | 影响 | 处理 |
|---|---|---|
| Mac 权限隐藏其他用户进程 | 扫描不完整 | 标注权限范围；无 sudo |
| 进程快速退出/重用 PID | 操作到错误进程 | 重验 PID/startTime/port；竞态仍有残余 |
| macOS `lsof` 格式与版本差异 | 解析错误 | 使用 -F0 + macOS fixture；真实系统交叉对照 |
| HTTP 检查命中代理/外部地址 | 安全问题 | no_proxy、禁重定向、精确 URL host 和回环连接限制 |
| 健康 URL 返回 200 但业务异常 | 假健康 | UI 精确称为“HTTP 2xx”，不承诺业务健康 |
| 某进程忽略 SIGTERM | 用户误解 | signalSent 与 processExited 分离，不升级 SIGKILL |
| GUI PATH 和开发终端不同 | 打包后扫描失败 | 使用固定系统 lsof 路径、安装版验证 |
| Store 数据损坏 | 收藏消失 | 不静默覆盖、提示恢复 |
| macOS Gatekeeper | 安装提醒 | 本地测试与面向公众分发区分，后续配置签名公证 |

## 3. 实现时需真机验证的细节

1. 当前 macOS 的 `/usr/sbin/lsof` 是否接受指定 `-F0pcftn` 与 `+c 0` 参数，真实 NUL/记录分隔形态是什么。
2. 当前锁定 `sysinfo` 版本在 Mac 上的 `start_time()/exe()/kill_with(Signal::Term)` 行为。
3. `localhost` 仅连接 loopback 的具体实现和单元/集成测试。
4. 是否需要调整 Tauri Store 默认 permissions 以适配最终配置。
5. 在未签名/自签名本地打包时 macOS 版本产生的具体 Gatekeeper 提示。

上述内容**不是开放式产品需求**。Codex 应在实现时查证官方 API 和真机结果，必要时对内部实现做最小兼容调整，但不得静默改动冻结的外部业务契约。

## 4. 版本边界

- v0.1：扫描 + 收藏 + HTTP 检查 + 安全 SIGTERM + DMG。
- v0.2 候选：菜单栏 / 全局快捷键。
- v0.3 候选：根据进程路径有限识别开发项目；不保证识别所有框架。
- v0.4 候选：用户显式配置的启动命令（有更大安全风险，需另立需求）。

**明确停止条件：** Tauri 安装版通过 Release Gate 后即发布 v0.1；不“顺便”扩展 Docker、SSH、日志系统、AI 代理等功能。
