# Codex 用量

> Codex usage 接口是未公开接口，可能随 OpenAI 调整而变化。

## 读取内容

Codex 用量只读取 Codex 相关数据：

- 5 小时 session 使用率。
- 7 天 weekly 使用率。
- code review 使用率。
- credits 余额和按 `$0.04` 估算的美元价值。
- reset credits 可用数量。
- 本地 `ccusage` 汇总的今日、昨日、近 30 天 token 和模型分布。

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

`prolite` 显示为 `Pro 5x`，`pro` 显示为 `Pro 20x`。credits 余额会向下取整展示，美元价值按每 credit `$0.04` 计算。

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
