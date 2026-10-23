# DevDock

macOS 本机 TCP 端口工具，使用 Tauri 2、Vue 3、TypeScript 和 Rust。

当前阶段：P1 端口扫描。Rust 已提供真实 TCP LISTEN 扫描、进程元数据和 `list_listeners` IPC。页面仍为基础窗口，P2 接入列表与详情；收藏、健康检查和进程终止尚未实现。

开发要求：macOS、Xcode Command Line Tools、Node.js 24、pnpm 11.5.2、Rust stable（锁定的 sysinfo 要求 Rust ≥1.95）。使用 Homebrew 安装 rustup 时，需将其工具目录加入 PATH：`export PATH="$(brew --prefix rustup)/bin:$PATH"`。

```sh
nvm use
pnpm install --frozen-lockfile
pnpm tauri dev
```

构建与检查：

```sh
pnpm build
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
