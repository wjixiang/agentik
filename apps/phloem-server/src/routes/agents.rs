//! Agent management endpoints: CRUD, lifecycle control.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, State},
    routing::{delete, get, post},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::state::AppState;

/// POST /api/agents — create a new agent.
async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<Json<AgentDetail>, AppError> {
    let handle = crate::services::agent_manager::get_or_create_agent(
        &state,
        Some(Uuid::new_v4()),
        Some(&req.identity),
    )
    .await?;

    Ok(Json(AgentDetail {
        id: handle.id.to_string(),
        identity: req.identity,
        status: handle.status().to_string(),
    }))
}

/// DELETE /api/agents/:agent_id — stop and remove an agent.
async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    crate::services::agent_manager::remove_agent(&state, agent_id).await?;
    Ok(Json(serde_json::json!({ "status": "removed" })))
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentRequest {
    pub identity: String,
}

#[derive(Debug, Serialize)]
pub struct AgentDetail {
    pub id: String,
    pub identity: String,
    pub status: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/agents", post(create_agent))
        .route("/api/agents/{agent_id}", delete(delete_agent))
}
