# Codex 用量

> Codex usage 接口是未公开接口，可能随 OpenAI 调整而变化。

## 读取内容

Codex 用量只读取 Codex 相关数据：

- 7 天 weekly 使用率，作为面板里的主限额。
- 5 小时 session 使用率，作为面板里的次级限额。
- 订阅类型，例如 `Pro 5x` 或 `Pro 20x`。
- reset credits 可用数量，显示为“可重置 N 次”。
- 本地 `ccusage` 汇总的今日、近 7 天、近 30 天 token 和美元价格。
- 本地 `ccusage` 汇总的模型分布。

远程限额会显示剩余额度、已用比例、重置时间，以及当前消耗相对线性时间进度是更快还是更慢。重置时间支持在“剩余时间”和“固定时间”两种展示方式之间切换。

面板不展示 credits 余额、credits 美元价值和 code review 限额。

## 面板行为

macOS 使用菜单栏 NSPanel。Windows 使用不显示任务栏图标的临时托盘窗口，从托盘图标附近弹出，窗口失焦后自动隐藏。托盘左右键点击都只切换面板，不显示右键菜单。

## 远程接口

```text
GET https://chatgpt.com/backend-api/wham/usage
```

请求头：

| 请求头 | 是否必需 | 值 |
|---|---|---|
| Authorization | yes | `Bearer <access_token>` |
| Accept | yes | `application/json` |
| ChatGPT-Account-Id | no | `<account_id>` |

响应里的主要字段：

```jsonc
{
  "plan_type": "prolite",
  "rate_limit": {
    "primary_window": {
      "used_percent": 6,
      "reset_at": 1780000000,
      "limit_window_seconds": 18000
    },
    "secondary_window": {
      "used_percent": 24,
      "reset_after_seconds": 120,
      "limit_window_seconds": 604800
    }
  },
  "code_review_rate_limit": {
    "primary_window": {
      "used_percent": 3,
      "reset_at": 1780600000,
      "limit_window_seconds": 604800
    }
  },
  "credits": {
    "balance": 820.6969075
  },
  "rate_limit_reset_credits": {
    "available_count": 1
  }
}
```

`prolite` 显示为 `Pro 5x`，`pro` 显示为 `Pro 20x`。面板只展示订阅类型和可重置次数，不展示 credits。

## 登录文件

当前实现只读取 Codex CLI 文件凭据，不读取 macOS keychain 或 Windows Credential Manager。

读取顺序：

1. `CODEX_HOME/auth.json`，当 `CODEX_HOME` 存在时只读这个位置。
2. `~/.config/codex/auth.json`
3. `~/.codex/auth.json`

期望文件结构：

```jsonc
{
  "OPENAI_API_KEY": null,
  "tokens": {
    "access_token": "<jwt>",
    "refresh_token": "<token>",
    "id_token": "<jwt>",
    "account_id": "<uuid>"
  },
  "last_refresh": "2026-01-28T08:05:37Z"
}
```

如果文件里只有 `OPENAI_API_KEY`，应用会报错，因为 Codex usage 需要 ChatGPT OAuth token。

## Token 刷新

当 usage 请求返回 401 或 403 时，应用会用 refresh token 请求：

```text
POST https://auth.openai.com/oauth/token
Content-Type: application/x-www-form-urlencoded
```

```text
grant_type=refresh_token
&client_id=app_EMoamEEZ73f0CkXaXp7hrann
&refresh_token=<refresh_token>
```

刷新成功后会把新的 token 写回原 `auth.json`。刷新失败时，界面会提示重新运行 `codex login`。

## 本地 ccusage

应用不会打包 `ccusage`。运行时按顺序查找这些 runner：

1. `bunx`
2. `pnpm dlx`
3. `yarn dlx`
4. `npm exec`
5. `npx`

找到后执行：

```bash
ccusage@20.0.2 codex daily --json --order desc
```

如果没有可用 runner，远程 usage 仍会显示，本地 usage 区域会标记为 runner 缺失。
