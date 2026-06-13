# Codex 用量

Codex 用量是一个个人用的 Tauri 桌面面板，只统计 OpenAI Codex 的使用情况。

它保留了原项目里对接 Codex usage 的核心逻辑，删除了多 provider 插件系统、本地 HTTP API、自动更新、分析上报、代理设置、全局快捷键和复杂设置页。

## 支持范围

- **支持平台：** macOS 和 Windows。
- **支持 provider：** 仅 Codex。
- **远程指标：** Codex session、weekly、code review、credits、reset credits。
- **本地指标：** 通过 `ccusage codex daily --json` 读取本地 token 和模型分布。
- **登录来源：** 读取 Codex CLI 的文件凭据。
- **托盘行为：** macOS 使用原来的 NSPanel 菜单栏面板；Windows 使用以托盘图标为中心、优先向上弹出的普通托盘窗口。

## 使用前提

先用 Codex CLI 登录，并确保凭据写入文件：

```bash
codex login
```

当前版本只读取文件凭据：

- `CODEX_HOME/auth.json`，当 `CODEX_HOME` 存在时只读这个位置。
- `~/.config/codex/auth.json`
- `~/.codex/auth.json`

如果你使用 Codex keyring/auto/ephemeral 凭据模式，需要切回文件凭据后再使用这个面板。

本地 token 统计需要系统里能运行 `bunx`、`pnpm dlx`、`yarn dlx`、`npm exec` 或 `npx` 之一。应用不会内置 `ccusage`，运行时会按需调用 `ccusage@20.0.2`。

## 开发

```bash
bun install
bun run dev
bun run tauri dev
```

常用验证：

```bash
bun run build
bun run test --run
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo check --manifest-path src-tauri/Cargo.toml
```

Windows 构建需要 Visual Studio Build Tools C++ 工具链。普通 PowerShell 没有 `link.exe` 时，用 Developer Command Prompt 或先加载 `VsDevCmd.bat`。

## 构建

```bash
bun run tauri build
```

macOS 仍保留原 NSPanel 实现。Windows 没有 NSPanel，对应实现会退化为普通 Tauri 窗口，由托盘菜单打开或隐藏，并在打开时从托盘上方向上滑出。

## 日志

日志路径见 [docs/capture-logs.md](docs/capture-logs.md)。

前端的 `console.warn` 和 `console.error` 会转发到 Tauri 日志，后端请求和 `ccusage` 失败会写入日志，同时在界面显示友好的错误信息。

## 许可

[MIT](LICENSE)
