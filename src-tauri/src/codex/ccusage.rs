use crate::codex::model::{CodexDayUsage, CodexLocalUsageSummary, CodexModelUsage};
use serde_json::Value;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CCUSAGE_VERSION: &str = "20.0.2";
const CCUSAGE_TIMEOUT: Duration = Duration::from_secs(15);
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RunnerKind {
    Bunx,
    PnpmDlx,
    YarnDlx,
    NpmExec,
    Npx,
}

pub fn query_local_usage() -> Result<CodexLocalUsageSummary, String> {
    let today = today_key();
    let runners = collect_runners();
    if runners.is_empty() {
        return Err("no_runner".to_string());
    }

    for (kind, program) in runners {
        match run_ccusage(kind, &program) {
            Ok(value) => return collect_local_usage_summary(&value, &today),
            Err(error) => log::warn!("ccusage via {:?} failed: {}", kind, error),
        }
    }

    Err("runner_failed".to_string())
}

fn today_key() -> String {
    let date = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month() as u8,
        date.day()
    )
}

fn runner_order() -> [RunnerKind; 5] {
    [
        RunnerKind::Bunx,
        RunnerKind::PnpmDlx,
        RunnerKind::YarnDlx,
        RunnerKind::NpmExec,
        RunnerKind::Npx,
    ]
}

fn runner_candidates(kind: RunnerKind) -> Vec<String> {
    let mut candidates = Vec::new();
    let home = dirs::home_dir();
    if let Some(home) = home.as_ref() {
        match kind {
            RunnerKind::Bunx => {
                candidates.push(home.join(".bun/bin/bunx").to_string_lossy().to_string());
                candidates.push(home.join(".bun/bin/bunx.exe").to_string_lossy().to_string());
            }
            RunnerKind::PnpmDlx => {
                candidates.push(
                    home.join("AppData/Local/pnpm/pnpm.cmd")
                        .to_string_lossy()
                        .to_string(),
                );
                candidates.push(
                    home.join("AppData/Roaming/npm/pnpm.cmd")
                        .to_string_lossy()
                        .to_string(),
                );
            }
            RunnerKind::YarnDlx => {
                candidates.push(
                    home.join("AppData/Roaming/npm/yarn.cmd")
                        .to_string_lossy()
                        .to_string(),
                );
            }
            RunnerKind::NpmExec => {
                candidates.push(
                    home.join("AppData/Roaming/npm/npm.cmd")
                        .to_string_lossy()
                        .to_string(),
                );
            }
            RunnerKind::Npx => {
                candidates.push(
                    home.join("AppData/Roaming/npm/npx.cmd")
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }
    }

    match kind {
        RunnerKind::Bunx => candidates.extend(["bunx", "bunx.exe"].map(str::to_string)),
        RunnerKind::PnpmDlx => candidates.extend(["pnpm", "pnpm.cmd"].map(str::to_string)),
        RunnerKind::YarnDlx => candidates.extend(["yarn", "yarn.cmd"].map(str::to_string)),
        RunnerKind::NpmExec => candidates.extend(["npm", "npm.cmd"].map(str::to_string)),
        RunnerKind::Npx => candidates.extend(["npx", "npx.cmd"].map(str::to_string)),
    }

    dedupe(candidates)
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        if value.trim().is_empty() || out.iter().any(|existing| existing == &value) {
            continue;
        }
        out.push(value);
    }
    out
}

fn path_entries_with(home: Option<&Path>, existing_path: Option<&OsStr>) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    if let Some(home) = home {
        entries.push(home.join(".bun/bin"));
        entries.push(home.join(".local/bin"));
        entries.push(home.join("AppData/Local/pnpm"));
        entries.push(home.join("AppData/Roaming/npm"));
    }
    entries.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    if let Some(existing_path) = existing_path {
        entries.extend(std::env::split_paths(existing_path));
    }
    let mut out = Vec::new();
    for entry in entries {
        if !entry.as_os_str().is_empty() && !out.iter().any(|existing| existing == &entry) {
            out.push(entry);
        }
    }
    out
}

fn enriched_path() -> Option<OsString> {
    std::env::join_paths(path_entries_with(
        dirs::home_dir().as_deref(),
        std::env::var_os("PATH").as_deref(),
    ))
    .ok()
}

fn runner_available(candidate: &str, path: Option<&OsStr>) -> bool {
    let mut command = Command::new(candidate);
    command.arg("--version");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    hide_command_window(&mut command);
    command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn collect_runners() -> Vec<(RunnerKind, String)> {
    let path = enriched_path();
    let mut out = Vec::new();
    for kind in runner_order() {
        for candidate in runner_candidates(kind) {
            if runner_available(&candidate, path.as_deref()) {
                out.push((kind, candidate));
                break;
            }
        }
    }
    out
}

fn runner_args(kind: RunnerKind) -> Vec<String> {
    let package = format!("ccusage@{CCUSAGE_VERSION}");
    let mut args = match kind {
        RunnerKind::Bunx => vec!["--silent".to_string(), package],
        RunnerKind::PnpmDlx => vec!["-s".to_string(), "dlx".to_string(), package],
        RunnerKind::YarnDlx => vec!["dlx".to_string(), "-q".to_string(), package],
        RunnerKind::NpmExec => vec![
            "exec".to_string(),
            "--yes".to_string(),
            format!("--package={package}"),
            "--".to_string(),
            "ccusage".to_string(),
        ],
        RunnerKind::Npx => vec!["--yes".to_string(), package],
    };

    args.extend([
        "codex".to_string(),
        "daily".to_string(),
        "--json".to_string(),
        "--order".to_string(),
        "desc".to_string(),
    ]);
    args
}

fn run_ccusage(kind: RunnerKind, program: &str) -> Result<Value, String> {
    let mut command = Command::new(program);
    command.args(runner_args(kind));
    if let Some(path) = enriched_path() {
        command.env("PATH", path);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_command_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn failed: {error}"))?;
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| format!("wait failed: {error}"))?;
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
                }
                let stdout = String::from_utf8_lossy(&output.stdout);
                let normalized = normalize_ccusage_output(&stdout)
                    .ok_or_else(|| "output parse failed".to_string())?;
                return serde_json::from_str(&normalized)
                    .map_err(|error| format!("normalized JSON parse failed: {error}"));
            }
            Ok(None) => {
                if start.elapsed() > CCUSAGE_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("wait failed: {error}")),
        }
    }
}

#[cfg(target_os = "windows")]
fn hide_command_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_command_window(_command: &mut Command) {}

pub fn normalize_ccusage_output(stdout: &str) -> Option<String> {
    let json = extract_last_json_value(stdout)?;
    let parsed: Value = serde_json::from_str(&json).ok()?;
    let normalized = match parsed {
        Value::Array(daily) => serde_json::json!({ "daily": daily }),
        Value::Object(map) => {
            if !map.get("daily").is_some_and(Value::is_array) {
                return None;
            }
            Value::Object(map)
        }
        _ => return None,
    };
    serde_json::to_string(&normalized).ok()
}

fn extract_last_json_value(stdout: &str) -> Option<String> {
    let trimmed = stdout.trim();
    if serde_json::from_str::<Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }
    let mut starts: Vec<usize> = trimmed
        .char_indices()
        .filter(|(_, ch)| *ch == '{' || *ch == '[')
        .map(|(idx, _)| idx)
        .collect();
    starts.reverse();
    for start in starts {
        let candidate = trimmed[start..].trim();
        if serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    None
}

pub fn collect_local_usage_summary(
    data: &Value,
    today: &str,
) -> Result<CodexLocalUsageSummary, String> {
    let daily = data
        .get("daily")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing daily usage".to_string())?;
    let yesterday = previous_day_key(today).unwrap_or_default();

    let mut today_usage = CodexDayUsage::empty("今日");
    let mut yesterday_usage = CodexDayUsage::empty("昨日");
    let mut last_30_days = CodexDayUsage::empty("近 30 天");
    let mut model_tokens = std::collections::BTreeMap::<String, u64>::new();
    let mut total_model_tokens = 0_u64;

    for day in daily {
        let Some(key) = day.get("date").and_then(Value::as_str).and_then(day_key) else {
            continue;
        };
        let tokens = number_as_u64(day.get("totalTokens")).unwrap_or(0);
        let cost_usd =
            number_as_f64(day.get("totalCost")).or_else(|| number_as_f64(day.get("costUSD")));
        if key == today {
            today_usage.tokens = tokens;
            today_usage.cost_usd = cost_usd;
        }
        if key == yesterday {
            yesterday_usage.tokens = tokens;
            yesterday_usage.cost_usd = cost_usd;
        }
        last_30_days.tokens = last_30_days.tokens.saturating_add(tokens);
        if let Some(cost) = cost_usd {
            last_30_days.cost_usd = Some(last_30_days.cost_usd.unwrap_or(0.0) + cost);
        }

        if let Some(models) = day.get("models").and_then(Value::as_object) {
            for (name, usage) in models {
                let tokens = model_token_count(usage);
                if tokens == 0 {
                    continue;
                }
                *model_tokens.entry(name.clone()).or_default() += tokens;
                total_model_tokens = total_model_tokens.saturating_add(tokens);
            }
        }
    }

    let mut models: Vec<CodexModelUsage> = model_tokens
        .into_iter()
        .map(|(name, tokens)| CodexModelUsage {
            name,
            tokens,
            percent: if total_model_tokens == 0 {
                0.0
            } else {
                (tokens as f64 / total_model_tokens as f64) * 100.0
            },
        })
        .collect();
    models.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));

    Ok(CodexLocalUsageSummary {
        today: today_usage,
        yesterday: yesterday_usage,
        last_30_days,
        models,
    })
}

impl CodexDayUsage {
    fn empty(label: &str) -> Self {
        Self {
            label: label.to_string(),
            tokens: 0,
            cost_usd: None,
        }
    }
}

fn model_token_count(value: &Value) -> u64 {
    if let Some(total) = number_as_u64(value.get("totalTokens")) {
        return total;
    }
    [
        "inputTokens",
        "cachedInputTokens",
        "cacheCreationTokens",
        "cacheReadTokens",
        "outputTokens",
        "reasoningOutputTokens",
    ]
    .into_iter()
    .filter_map(|field| number_as_u64(value.get(field)))
    .sum()
}

fn number_as_u64(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    if let Some(n) = value.as_u64() {
        return Some(n);
    }
    value
        .as_f64()
        .filter(|n| n.is_finite() && *n >= 0.0)
        .map(|n| n as u64)
}

fn number_as_f64(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}

fn day_key(raw: &str) -> Option<String> {
    let value = raw.trim();
    if value.len() >= 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
    {
        return Some(value[..10].to_string());
    }
    if value.len() == 8 && value.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(format!(
            "{}-{}-{}",
            &value[0..4],
            &value[4..6],
            &value[6..8]
        ));
    }
    None
}

fn previous_day_key(today: &str) -> Option<String> {
    let parts: Vec<_> = today.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year = parts[0].parse().ok()?;
    let month = match parts[1].parse::<u8>().ok()? {
        1 => time::Month::January,
        2 => time::Month::February,
        3 => time::Month::March,
        4 => time::Month::April,
        5 => time::Month::May,
        6 => time::Month::June,
        7 => time::Month::July,
        8 => time::Month::August,
        9 => time::Month::September,
        10 => time::Month::October,
        11 => time::Month::November,
        12 => time::Month::December,
        _ => return None,
    };
    let day = parts[2].parse().ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let previous = date.previous_day()?;
    Some(format!(
        "{:04}-{:02}-{:02}",
        previous.year(),
        previous.month() as u8,
        previous.day()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_ccusage_output_converts_array_to_daily_object() {
        let normalized = normalize_ccusage_output("noise\n[]\n").expect("normalized output");
        let value: serde_json::Value = serde_json::from_str(&normalized).expect("valid json");

        assert_eq!(value, serde_json::json!({ "daily": [] }));
    }

    #[test]
    fn collect_local_usage_summary_reads_today_yesterday_total_and_models() {
        let daily = serde_json::json!({
            "daily": [
                {
                    "date": "2026-06-12",
                    "totalTokens": 1000,
                    "costUSD": 0.5,
                    "models": {
                        "gpt-5.5": { "totalTokens": 1000 }
                    }
                },
                {
                    "date": "2026-06-13",
                    "totalTokens": 3000,
                    "costUSD": 1.25,
                    "models": {
                        "gpt-5.5": { "totalTokens": 2000 },
                        "gpt-5.4": { "inputTokens": 700, "outputTokens": 300 }
                    }
                }
            ]
        });

        let summary = collect_local_usage_summary(&daily, "2026-06-13").expect("summary");

        assert_eq!(summary.today.tokens, 3000);
        assert_eq!(summary.yesterday.tokens, 1000);
        assert_eq!(summary.last_30_days.tokens, 4000);
        assert_eq!(summary.models[0].name, "gpt-5.5");
        assert_eq!(summary.models[0].tokens, 3000);
        assert_eq!(summary.models[1].name, "gpt-5.4");
        assert_eq!(summary.models[1].tokens, 1000);
    }
}
