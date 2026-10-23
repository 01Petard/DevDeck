# DevDock

macOS 本机 TCP 端口工具，使用 Tauri 2、Vue 3、TypeScript 和 Rust。

当前阶段：P0 工程初始化。已提供原生窗口和基础样式，尚未接入扫描、收藏、健康检查或进程终止。

开发要求：macOS、Xcode Command Line Tools、Node.js 24、pnpm 11.5.2、Rust stable。使用 Homebrew 安装 rustup 时，需将其工具目录加入 PATH：`export PATH="$(brew --prefix rustup)/bin:$PATH"`。

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

下一阶段：P1 真实 lsof 扫描与进程信息。
