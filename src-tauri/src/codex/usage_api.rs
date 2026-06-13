use crate::codex::auth::{self, AuthState};
use crate::codex::model::{CodexMetric, CodexUsageSnapshot};
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashMap;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const CREDIT_USD_RATE: f64 = 0.04;
const PERIOD_SESSION_MS: u64 = 5 * 60 * 60 * 1000;
const PERIOD_WEEKLY_MS: u64 = 7 * 24 * 60 * 60 * 1000;

pub fn load_remote_usage() -> Result<CodexUsageSnapshot, String> {
    let mut state = auth::find_auth_state()?;
    let client = Client::builder()
        .user_agent("Codex Usage")
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败：{error}"))?;

    let mut response = fetch_usage(&client, &state)?;
    if response.status == 401 || response.status == 403 {
        refresh_auth(&client, &mut state)?;
        response = fetch_usage(&client, &state)?;
    }

    if response.status == 401 || response.status == 403 {
        return Err("Codex 会话已过期，请重新运行 `codex login`。".to_string());
    }
    if !(200..300).contains(&response.status) {
        return Err(format!("Codex 用量请求失败（HTTP {}）。", response.status));
    }

    let body: Value = serde_json::from_str(&response.body)
        .map_err(|error| format!("Codex 用量响应不是有效 JSON：{error}"))?;
    parse_usage_response(
        &body,
        &response.headers,
        now_unix_seconds(),
        &now_iso_string(),
    )
}

struct UsageHttpResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

fn fetch_usage(client: &Client, state: &AuthState) -> Result<UsageHttpResponse, String> {
    let tokens = state
        .auth
        .tokens
        .as_ref()
        .ok_or_else(|| "Codex 用量需要 ChatGPT 登录。".to_string())?;
    let mut request = client
        .get(USAGE_URL)
        .bearer_auth(&tokens.access_token)
        .header("Accept", "application/json");
    if let Some(account_id) = tokens
        .account_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request
        .send()
        .map_err(|error| format!("Codex 用量请求失败，请检查网络连接：{error}"))?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(key, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (key.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect();
    let body = response
        .text()
        .map_err(|error| format!("读取 Codex 用量响应失败：{error}"))?;
    Ok(UsageHttpResponse {
        status,
        headers,
        body,
    })
}

fn refresh_auth(client: &Client, state: &mut AuthState) -> Result<(), String> {
    let refresh_token = state
        .auth
        .tokens
        .as_ref()
        .and_then(|tokens| tokens.refresh_token.as_deref())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Codex 会话已过期，请重新运行 `codex login`。".to_string())?
        .to_string();

    let response = client
        .post(REFRESH_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .map_err(|error| format!("Codex token 刷新失败：{error}"))?;
    let status = response.status().as_u16();
    let body: Value = response
        .json()
        .map_err(|error| format!("Codex token 刷新响应无效：{error}"))?;

    if !(200..300).contains(&status) {
        let code = body
            .pointer("/error/code")
            .and_then(Value::as_str)
            .or_else(|| body.get("error").and_then(Value::as_str))
            .unwrap_or("unknown");
        return Err(match code {
            "refresh_token_expired" => "Codex 会话已过期，请重新运行 `codex login`。".to_string(),
            "refresh_token_reused" => "Codex token 冲突，请重新运行 `codex login`。".to_string(),
            "refresh_token_invalidated" => {
                "Codex token 已撤销，请重新运行 `codex login`。".to_string()
            }
            _ => "Codex token 已过期，请重新运行 `codex login`。".to_string(),
        });
    }

    let access_token = body
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Codex token 刷新响应缺少 access_token。".to_string())?
        .to_string();
    let tokens = state
        .auth
        .tokens
        .as_mut()
        .ok_or_else(|| "Codex 用量需要 ChatGPT 登录。".to_string())?;
    tokens.access_token = access_token;
    if let Some(refresh_token) = body.get("refresh_token").and_then(Value::as_str) {
        tokens.refresh_token = Some(refresh_token.to_string());
    }
    if let Some(id_token) = body.get("id_token").and_then(Value::as_str) {
        tokens.id_token = Some(id_token.to_string());
    }
    state.auth.last_refresh = Some(now_iso_string());
    auth::save_auth_state(state)
}

pub fn parse_usage_response(
    data: &Value,
    headers: &HashMap<String, String>,
    now_sec: i64,
    fetched_at: &str,
) -> Result<CodexUsageSnapshot, String> {
    let rate_limit = data.get("rate_limit").unwrap_or(&Value::Null);
    let primary_window = rate_limit.get("primary_window").unwrap_or(&Value::Null);
    let secondary_window = rate_limit.get("secondary_window").unwrap_or(&Value::Null);
    let review_window = data
        .pointer("/code_review_rate_limit/primary_window")
        .unwrap_or(&Value::Null);

    let session_used = header_percent(headers, "x-codex-primary-used-percent")
        .or_else(|| percent_from_window(primary_window));
    let weekly_used = header_percent(headers, "x-codex-secondary-used-percent")
        .or_else(|| percent_from_window(secondary_window));
    let reviews_used = percent_from_window(review_window);

    Ok(CodexUsageSnapshot {
        plan: data
            .get("plan_type")
            .and_then(Value::as_str)
            .and_then(format_codex_plan),
        session: session_used.map(|used_percent| CodexMetric {
            label: "5 小时限额".to_string(),
            used_percent,
            resets_at: reset_at_iso(primary_window, now_sec),
            period_duration_ms: Some(period_duration_ms(primary_window, PERIOD_SESSION_MS)),
        }),
        weekly: weekly_used.map(|used_percent| CodexMetric {
            label: "周限额".to_string(),
            used_percent,
            resets_at: reset_at_iso(secondary_window, now_sec),
            period_duration_ms: Some(period_duration_ms(secondary_window, PERIOD_WEEKLY_MS)),
        }),
        reviews: reviews_used.map(|used_percent| CodexMetric {
            label: "代码评审".to_string(),
            used_percent,
            resets_at: reset_at_iso(review_window, now_sec),
            period_duration_ms: Some(period_duration_ms(review_window, PERIOD_WEEKLY_MS)),
        }),
        credits_remaining: read_credits_remaining(data, headers),
        credits_usd: read_credits_remaining(data, headers)
            .map(|credits| ((credits as f64 * CREDIT_USD_RATE) * 100.0).round() / 100.0),
        reset_credits_available: data
            .pointer("/rate_limit_reset_credits/available_count")
            .and_then(number_as_u64),
        local_usage: None,
        local_usage_status: "not_checked".to_string(),
        fetched_at: fetched_at.to_string(),
    })
}

fn header_percent(headers: &HashMap<String, String>, key: &str) -> Option<f64> {
    headers.get(key).and_then(|value| value.parse::<f64>().ok())
}

fn percent_from_window(window: &Value) -> Option<f64> {
    window.get("used_percent").and_then(number_as_f64)
}

fn number_as_f64(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}

fn number_as_u64(value: &Value) -> Option<u64> {
    if let Some(value) = value.as_u64() {
        return Some(value);
    }
    value
        .as_f64()
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number.floor() as u64)
}

fn read_credits_remaining(data: &Value, headers: &HashMap<String, String>) -> Option<u64> {
    if let Some(balance) = data.pointer("/credits/balance").and_then(number_as_u64) {
        return Some(balance);
    }
    if data
        .pointer("/credits/has_credits")
        .and_then(Value::as_bool)
        == Some(false)
    {
        return Some(0);
    }
    headers
        .get("x-codex-credits-balance")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| number.floor() as u64)
}

fn reset_at_iso(window: &Value, now_sec: i64) -> Option<String> {
    if let Some(reset_at) = window.get("reset_at").and_then(number_as_i64) {
        return Some(unix_seconds_to_iso(reset_at));
    }
    window
        .get("reset_after_seconds")
        .and_then(number_as_i64)
        .map(|seconds| unix_seconds_to_iso(now_sec + seconds))
}

fn number_as_i64(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .map(|number| number as i64)
}

fn period_duration_ms(window: &Value, fallback: u64) -> u64 {
    window
        .get("limit_window_seconds")
        .and_then(number_as_u64)
        .and_then(|seconds| seconds.checked_mul(1000))
        .unwrap_or(fallback)
}

fn format_codex_plan(plan_type: &str) -> Option<String> {
    let trimmed = plan_type.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "prolite" => Some("Pro 5x".to_string()),
        "pro" => Some("Pro 20x".to_string()),
        _ => {
            let mut chars = trimmed.chars();
            let first = chars.next()?.to_uppercase().to_string();
            Some(format!("{}{}", first, chars.as_str()))
        }
    }
}

fn unix_seconds_to_iso(seconds: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn now_unix_seconds() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn now_iso_string() -> String {
    unix_seconds_to_iso(now_unix_seconds())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parse_usage_response_prefers_headers_and_reads_detail_metrics() {
        let body = serde_json::json!({
            "plan_type": "prolite",
            "rate_limit": {
                "primary_window": { "used_percent": 6, "reset_at": 1780000000, "limit_window_seconds": 18000 },
                "secondary_window": { "used_percent": 24, "reset_after_seconds": 120, "limit_window_seconds": 604800 }
            },
            "code_review_rate_limit": {
                "primary_window": { "used_percent": 3, "reset_at": 1780600000, "limit_window_seconds": 604800 }
            },
            "credits": { "balance": 820.6969075 },
            "rate_limit_reset_credits": { "available_count": 1 }
        });
        let mut headers = HashMap::new();
        headers.insert("x-codex-primary-used-percent".to_string(), "7".to_string());
        headers.insert(
            "x-codex-secondary-used-percent".to_string(),
            "25".to_string(),
        );

        let snapshot = parse_usage_response(&body, &headers, 1_700_000_000, "2026-06-13T12:00:00Z")
            .expect("snapshot");

        assert_eq!(snapshot.plan.as_deref(), Some("Pro 5x"));
        assert_eq!(snapshot.session.expect("session").used_percent, 7.0);
        assert_eq!(snapshot.weekly.expect("weekly").used_percent, 25.0);
        assert_eq!(snapshot.reviews.expect("reviews").used_percent, 3.0);
        assert_eq!(snapshot.credits_remaining, Some(820));
        assert_eq!(snapshot.credits_usd, Some(32.8));
        assert_eq!(snapshot.reset_credits_available, Some(1));
        assert_eq!(snapshot.local_usage_status, "not_checked");
    }
}
