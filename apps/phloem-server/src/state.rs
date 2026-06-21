//! Shared application state passed to all axum handlers via `State<AppState>`.

use std::collections::HashMap;

use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use agentik_sdk::types::AgentEvent;

use crate::config::ServerConfig;
use crate::services::agent_manager::AgentHandle;

/// Global application state shared across all request handlers.
pub struct AppState {
    /// Server configuration.
    pub config: ServerConfig,
    /// Running agent instances, keyed by their UUID.
    pub agents: RwLock<HashMap<Uuid, AgentHandle>>,
    /// Global event bus — bridge tasks forward per-agent events here.
    /// Consumers (SSE subscribers) filter by agent UUID.
    pub event_broker: broadcast::Sender<(Uuid, AgentEvent)>,
}

impl AppState {
    pub fn new(config: ServerConfig) -> Self {
        let (event_broker, _) = broadcast::channel(1024);
        Self {
            config,
            agents: RwLock::new(HashMap::new()),
            event_broker,
        }
    }
}
