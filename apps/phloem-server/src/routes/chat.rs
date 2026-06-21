//! Chat endpoints: send messages and stream agent events.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    response::sse::Sse,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::sse::agent_event_stream;
use crate::state::AppState;

/// POST /api/chat — send a message to an agent (create if needed).
async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, AppError> {
    let handle = crate::services::agent_manager::get_or_create_agent(
        &state,
        req.agent_id,
        req.identity.as_deref(),
    )
    .await?;

    handle.send_message(&req.content).await?;

    Ok(Json(ChatResponse {
        agent_id: handle.id.to_string(),
        status: "ok".to_string(),
    }))
}

/// GET /api/chat/:agent_id/stream — SSE stream for a specific agent's events.
async fn stream_events(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<Uuid>,
) -> Result<Sse<impl futures::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>>, AppError>
    where
        std::convert::Infallible: Send,
{
    let rx = state.event_broker.subscribe();
    Ok(agent_event_stream(agent_id, rx))
}

/// GET /api/agents — list all running agents.
async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<AgentInfo>> {
    let agents = state.agents.read().await;
    let list: Vec<AgentInfo> = agents
        .values()
        .map(|h| AgentInfo {
            id: h.id.to_string(),
            identity: h.identity.clone(),
            status: h.status().to_string(),
        })
        .collect();
    Json(list)
}

// ── Request / Response types ──

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// Optional agent UUID. If None, a new agent is created.
    pub agent_id: Option<Uuid>,
    /// Optional identity string for new agents.
    pub identity: Option<String>,
    /// The user message content.
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub agent_id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub identity: String,
    pub status: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chat", post(send_message))
        .route("/api/chat/{agent_id}/stream", get(stream_events))
        .route("/api/agents", get(list_agents))
}
