//! Settings persistence service.

use crate::state::AppState;

/// Load settings from disk (JSON file).
pub async fn load_settings(_state: &AppState) -> anyhow::Result<serde_json::Value> {
    // TODO: read from data/settings.json
    Ok(serde_json::json!({}))
}

/// Save settings to disk.
pub async fn save_settings(_state: &AppState, _settings: serde_json::Value) -> anyhow::Result<()> {
    // TODO: write to data/settings.json
    Ok(())
}
