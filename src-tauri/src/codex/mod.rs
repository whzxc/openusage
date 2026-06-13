mod auth;
mod ccusage;
mod model;
mod usage_api;

pub use model::CodexUsageSnapshot;

pub fn load_usage_snapshot() -> Result<CodexUsageSnapshot, String> {
    let mut snapshot = usage_api::load_remote_usage()?;
    match ccusage::query_local_usage() {
        Ok(local_usage) => {
            snapshot.local_usage = Some(local_usage);
            snapshot.local_usage_status = "ok".to_string();
        }
        Err(error) => {
            log::warn!("Codex local token usage unavailable: {}", error);
            snapshot.local_usage = None;
            snapshot.local_usage_status = error;
        }
    }
    Ok(snapshot)
}
