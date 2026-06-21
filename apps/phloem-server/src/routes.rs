//! Route modules and top-level router composition.

mod agents;
mod chat;
pub(crate) mod lake;
mod settings;

use std::sync::Arc;

use axum::Router;

use crate::state::AppState;

/// Build the complete application router.
pub fn create_router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(chat::routes())
        .merge(agents::routes())
        .merge(lake::routes())
        .merge(settings::routes())
}
