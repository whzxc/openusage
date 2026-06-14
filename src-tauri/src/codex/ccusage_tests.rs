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
                "date": "2026-05-30",
                "totalTokens": 9000,
                "costUSD": 3.75,
                "models": {
                    "gpt-5.4": { "totalTokens": 9000 }
                }
            },
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
    assert_eq!(summary.last_7_days.tokens, 4000);
    assert_eq!(summary.last_30_days.tokens, 13000);
    assert_eq!(summary.models[0].name, "gpt-5.4");
    assert_eq!(summary.models[0].tokens, 10000);
    assert_eq!(summary.models[1].name, "gpt-5.5");
    assert_eq!(summary.models[1].tokens, 3000);
}
