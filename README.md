# DevDock

macOS 本机 TCP 端口工具，使用 Tauri 2、Vue 3、TypeScript 和 Rust。

当前阶段：P3 收藏与健康检查。支持真实 TCP LISTEN 扫描、搜索、进程详情、端口收藏及手动回环 HTTP 检测。安全终止进程尚未实现。

开发要求：macOS、Xcode Command Line Tools、Node.js 24、pnpm 11.5.2、Rust stable（锁定的 sysinfo 要求 Rust ≥1.95）。使用 Homebrew 安装 rustup 时，需将其工具目录加入 PATH：`export PATH="$(brew --prefix rustup)/bin:$PATH"`。

```sh
nvm use
pnpm install --frozen-lockfile
pnpm tauri dev
```

构建与检查：

```sh
pnpm build
pnpm test
cd src-tauri
cargo check --locked
```

开发阶段与后续验收要求见 [开发任务](docs/05_Codex_分阶段任务.md)。DMG 构建与安装验证属于 P5，当前尚未执行。

## P0 验证记录

2026-10-24，在 Apple Silicon Mac 上验证：

- `pnpm build`：PASS（退出码 0）。
- `cargo check --locked`：PASS（退出码 0，依赖下载使用下述临时镜像配置）。
- `pnpm tauri dev`：PASS（Rust 可执行文件已启动，原生窗口显示 DevDock 基础工作区，无模板欢迎页）。
- `pnpm tauri bundle --debug --bundles app --no-sign`：PASS（退出码 0）；通过打开调试 `.app` 完成原生窗口的可视检查，已确认当前系统深色主题下正常渲染。
- `cargo test`：NOT RUN（P0 尚无业务逻辑和单元测试）。
- DMG、安装版验收、签名及公证：NOT RUN（P5）。调试应用使用模板图标，仍依赖 Vite 开发服务，不能作为独立安装版分发。

已锁定 Tauri Rust 2.12.5、Tauri CLI/API 2.12.4、Vue 3.5.44、Vite 8.3.5、TypeScript 6.0.3；本机 Rust 为 1.99.1。锁文件为 `pnpm-lock.yaml` 和 `src-tauri/Cargo.lock`。

本机连接 crates.io 的包下载超时，因此验证时仅为当前命令指定镜像，未修改全局或项目下载源：

```sh
cd src-tauri
cargo --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' check --locked
cd ..
pnpm tauri dev -- --config 'source.crates-io.replace-with="rsproxy"' --config 'source.rsproxy.registry="sparse+https://rsproxy.cn/index/"' --locked
```

## P1 验证记录

2026-10-24，在同一 Apple Silicon Mac 上验证。新增依赖锁定为 sysinfo 0.39.6、Tokio 1.54.1；固定调用 macOS `/usr/sbin/lsof` 4.91，不使用 Shell。

- `cargo test --locked`：PASS（退出码 0，11 项通过；1 项占用固定端口的真机测试默认忽略并单独运行）。覆盖解析、IPv4/IPv6、多 Socket、去重排序、非法记录、空快照、结构化错误、输出上限、超时回收、扫描防并发和元数据降级。
- `cargo test --locked live_scan -- --ignored --nocapture --test-threads=1`：PASS（退出码 0，1 项通过）。本轮临时 Python 服务 PID 46655 的 `127.0.0.1:18765` 被原生命令发现，PID 与直接执行 lsof 一致，启动时间及可执行路径非空。仅对该自建服务发送 SIGTERM 清理，重扫记录消失，独立 lsof 检查确认 18765 无监听。
- `cargo check --locked`、`cargo fmt --check`：PASS（退出码 0）。
- `pnpm build`：PASS（退出码 0）。
- Rust 命令在 `src-tauri/` 下执行；下载依赖时沿用上述临时镜像参数，锁文件仍保留 crates.io 的版本与校验值。

边界：结果为当前权限可见的瞬时快照；不可获取的启动时间、路径为 null。lsof 报错或输出无法可靠解析时返回错误，不伪装为空列表；有 stderr 诊断时保守拒绝本次快照。stdout 上限 8 MiB、stderr 上限 8 KiB、lsof 执行超时 5 秒。真实 IPC 窗口交互验证随 P2 页面接入完成；健康与进程终止命令、DMG 安装版验收仍未执行。

下一阶段：P2 主界面、搜索与监听详情。

## P2 验证记录

2026-10-24，在同一 Mac 上完成主窗口接入。前端使用 Vue 响应式状态与原生 CSS，无 Router、Pinia、模拟端口或新增运行时依赖。

- `pnpm test`：PASS（退出码 0，7 项通过）。使用 Node 24 内置测试运行器，fixture 仅位于 `tests/`。覆盖搜索与排序、Socket/进程身份、真实空结果与失败区分、旧快照保留、失效选择清除、扫描防并发、300ms 手动防抖、2 秒焦点节流和 IPC 错误脱敏。
- `pnpm build`：PASS（退出码 0）。
- `pnpm tauri dev` 与调试 `.app`：PASS。原生窗口显示真实监听；按端口 18765、完整 PID 52404、混合大小写 `pYtHoN` 均过滤到本次自建 Python 服务。详情包含真实 PID、地址、启动时间及完整可执行路径。停止该自建服务后点击刷新，监听行与旧详情均消失；独立 lsof 检查确认临时端口已释放。
- 窗口重新聚焦后自动更新时间：PASS；节流边界另有自动化测试。当前系统深色主题下，1000×680 详情与 780×520 搜索空态完成可视检查，长路径换行、页面分栏和底栏未被裁切。
- `git diff --check`：PASS。Rust 代码与依赖未变，沿用 P1 的单元测试和检查结果；本轮原生启动重新编译成功。
- 扫描失败/旧快照/真实空结果已由状态测试覆盖；未修改用户服务来制造全机空结果。浅色主题未人工切换；浏览器错误态可视检查 NOT RUN（本次 IAB 工具不可用）。

界面将刷新中及失败快照标记为过期，禁止选择其中的旧进程；仅保留相同 PID、启动时间和 Socket 身份的有效选择。深浅主题使用系统媒体查询，无新系统设置。开发窗口和自建测试服务已退出；调试包仍依赖 Vite，DMG、签名及安装验收留在 P5。

下一阶段：P3 收藏端口与回环 HTTP 健康检查。

## P3 验证记录

2026-10-24，在同一 Apple Silicon Mac 上验证。新增依赖锁定为 Tauri Store 2.5.0（Rust/JS）、reqwest 0.13.5、url 2.5.8。

- `pnpm test`：PASS（退出码 0，16 项通过）。覆盖收藏端口唯一、Unicode 别名、有效 URL 端口、损坏/新版配置拒绝、保存失败回滚与重试、无监听收藏、无配置不请求、并发上限、重复点击及编辑 URL 时旧请求结果失效。
- `pnpm build`：PASS（退出码 0）。
- `cargo test --locked`：PASS（退出码 0，19 项通过，2 项默认忽略）。本轮新增 HTTP 测试使用真正的本机 TCP 服务，验证 200/204/301/404/500、只读响应头、localhost 回环解析、IPv6、TLS 失败、连接拒绝、3 秒超时、同 URL 防并发及最多 4 个请求。代理绕过测试在独立子进程设置代理环境，验证未连接测试代理；其中 1 项忽略测试仅作为该测试的子进程入口，已由父测试执行。另 1 项为 P1 已单独验证的真实扫描测试。
- `cargo check --locked`、`cargo fmt --check`、`git diff --check`：PASS（退出码 0）。Rust 命令沿用上述临时镜像配置。
- 原生窗口：PASS。自行启动的临时服务 PID 68294 / `127.0.0.1:18765`，分别手动检测 localhost 200、404、301、慢响应超时（界面显示 3001 ms）。测试服务日志仅出现手动请求路径，301 后未请求跳转目标。仅对该自建服务发送 SIGTERM 清理，刷新后收藏显示 TCP 未监听，HTTP 检查显示连接失败。
- 收藏保存、取消编辑、编辑、取消收藏、外部 URL 拒绝：PASS。退出并重启 `.app` 后仍保留别名、端口和 URL，健康结果恢复为“尚未检测”；不持久化 PID、扫描记录或健康历史。
- 损坏配置：PASS。本次创建的配置故意写入无效 JSON 后，应用显示明确错误并禁止保存；退出后原文件逐字节未变。修复文件并点击“重新读取”恢复收藏。Rust 预加载与序列化校验同时防止 Store 插件退出保存时覆盖损坏配置。
- 保存失败：PASS。将本次创建的配置临时设为只读，保存失败时保留草稿并显示提示，原文件逐字节未变；恢复写权限后重试成功。最后已取消测试收藏并恢复正常权限，开发应用和测试服务已清理。
- 深色主题下 1000×680 与最小 780×520 收藏详情、编辑对话框：PASS。浅色主题人工切换、DMG 和安装版验收：NOT RUN（后两项留在 P5）。

使用：点击“收藏端口”添加任意合法端口，或从监听详情收藏当前端口；相同端口会更新已有收藏。点击收藏可查看 TCP 状态及全部监听候选，点击具体候选查看进程。配置 URL 后点击“检测 HTTP”主动检测，顶部刷新只重扫 TCP。HTTP 2xx 仅表示该 URL 返回 2xx，不代表业务健康。

配置位于 `~/Library/Application Support/top.bugstack.devdock/devdock.json`，根键为 `settings`。配置不可读或版本不支持时，先备份并修复文件，再点击“重新读取”；如果选择移走损坏文件，重启应用可创建空配置。当前使用 Store 的显式保存，异常断电下不承诺文件写入原子性。不要同时运行多个开发实例编辑同一配置。

下一阶段：P4 安全 SIGTERM 操作。
