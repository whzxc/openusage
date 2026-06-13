# 捕获日志

当 Codex 用量没有正常显示数据时，用这个文件定位日志。

## macOS

日志通常在：

```text
~/Library/Logs/com.whzxc.codexusage/Codex 用量.log
```

也可以从界面底部看到日志已准备好的状态。当前精简版没有 Debug Level 菜单。

## Windows

日志通常在：

```text
%LOCALAPPDATA%\com.whzxc.codexusage\logs\Codex 用量.log
```

如果路径不存在，先启动一次应用并刷新面板。

## 排查时记录

复制错误信息时，保留这些上下文：

```text
发生时间：
系统：
Codex CLI 是否已登录：
是否设置 CODEX_HOME：
界面错误：
最近的日志行：
```

日志会记录请求失败、token 刷新失败和 `ccusage` runner 失败。公开分享前仍建议检查一遍日志内容。
