//! Settings endpoints: provider config, model pool management.

use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    routing::get,
    Json,
};

use crate::state::AppState;

/// GET /api/settings — return current server settings.
async fn get_settings(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: read from settings service once implemented
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/settings", get(get_settings))
}
