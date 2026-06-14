use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexMetric {
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<String>,
    pub period_duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexDayUsage {
    pub label: String,
    pub tokens: u64,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelUsage {
    pub name: String,
    pub tokens: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexLocalUsageSummary {
    pub today: CodexDayUsage,
    pub yesterday: CodexDayUsage,
    pub last_7_days: CodexDayUsage,
    pub last_30_days: CodexDayUsage,
    pub models: Vec<CodexModelUsage>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CodexUsageSnapshot {
    pub plan: Option<String>,
    pub session: Option<CodexMetric>,
    pub weekly: Option<CodexMetric>,
    pub reviews: Option<CodexMetric>,
    pub credits_remaining: Option<u64>,
    pub credits_usd: Option<f64>,
    pub reset_credits_available: Option<u64>,
    pub local_usage: Option<CodexLocalUsageSummary>,
    pub local_usage_status: String,
    pub fetched_at: String,
}
