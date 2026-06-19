//! REST + SSE control-plane HTTP server.
//!
//! [`router`] builds the axum app exposing the agent lifecycle / observation
//! API as JSON-over-HTTP, with an SSE endpoint for the event stream
//! (`/events`).
//!
//! Bodies use the serde wire types in `agentik-api` directly — no `*_json`
//! string wrapping.

use std::sync::Arc;

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use agentik_api::{
    AgentLifecycleStatus, AgentSpawnOpts, ContentBlock, ModelConfig, PoolEntry, ProcessEvent,
    ProcessExitStatus, ProviderConfig,
};

// `ImageSource` nests under `ContentBlock`; pull it in for the OpenAPI
// `components(schemas(...))` list.
#[allow(unused_imports)]
use agentik_types::ImageSource;

use crate::process::AgentManager;

/// Shared state for all handlers.
#[derive(Clone)]
pub struct HttpState {
    inner: Arc<HttpStateInner>,
}

struct HttpStateInner {
    pm: AgentManager,
    shutdown: CancellationToken,
}

impl HttpState {
    pub fn new(
        pm: AgentManager,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            inner: Arc::new(HttpStateInner {
                pm,
                shutdown,
            }),
        }
    }
}

/// OpenAPI document for the control-plane REST API, served at
/// `/api-docs/openapi.json` and rendered as Swagger UI at `/docs`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "agentik control plane",
        version = "0.1.0",
        description = "REST + SSE API for the agentik runtime daemon \
                       (agent lifecycle, model pool).",
    ),
    paths(
        health, shutdown,
        spawn_agent, list_agents, start_agent, stop_agent, restart_agent,
        inject_message, get_status, stream_events,
        pool_models, configure_pool, list_kinds,
    ),
    components(schemas(
        SpawnAgentBody, AgentIdBody, InjectMessageBody,
        AckBody, StatusBody, AgentInfoBody,
        AgentLifecycleStatus, AgentSpawnOpts, ContentBlock, ImageSource,
        ModelConfig, ProviderConfig, PoolEntry,
        ProcessEvent, ProcessExitStatus,
    )),
    tags(
        (name = "system", description = "health / shutdown"),
        (name = "agents", description = "agent lifecycle & observation"),
        (name = "pool",   description = "model pool / kinds"),
    ),
)]
pub struct ApiDoc;

/// Build the control-plane axum router.
pub fn router(state: HttpState) -> Router {
    let cors = CorsLayer::very_permissive();

    let swagger = SwaggerUi::new("/docs")
        .url("/api-docs/openapi.json", ApiDoc::openapi());

    Router::new()
        .merge(swagger)
        // ── health / lifecycle ──
        .route("/api/v1/health", get(health))
        .route("/api/v1/shutdown", post(shutdown))
        // ── agents ──
        .route("/api/v1/agents", post(spawn_agent).get(list_agents))
        .route("/api/v1/agents/{id}/start", post(start_agent))
        .route("/api/v1/agents/{id}/stop", post(stop_agent))
        .route("/api/v1/agents/{id}/restart", post(restart_agent))
        .route("/api/v1/agents/{id}/messages", post(inject_message))
        .route("/api/v1/agents/{id}/status", get(get_status))
        // ── pool / kinds ──
        .route("/api/v1/pool/models", get(pool_models))
        .route("/api/v1/pool", put(configure_pool))
        .route("/api/v1/kinds", get(list_kinds))
        // ── events (SSE) ──
        .route("/api/v1/events", get(stream_events))
        .layer(cors)
        .with_state(state)
}

// ── Request / response bodies ───────────────────────────────────

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct SpawnAgentBody {
    kind: String,
    #[serde(default)]
    opts: AgentSpawnOpts,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct AgentIdBody {
    agent_id: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
struct InjectMessageBody {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct AckBody {
    ok: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    error: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct StatusBody {
    status: AgentLifecycleStatus,
}

fn ack(ok: bool, error: impl Into<String>) -> AckBody {
    AckBody {
        ok,
        error: error.into(),
    }
}

fn ok_ack() -> AckBody {
    ack(true, "")
}

fn err_status<E: std::fmt::Display>(code: StatusCode, e: E) -> (StatusCode, String) {
    (code, e.to_string())
}

// ── Handlers ────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses((status = 200, body = AckBody, description = "daemon is alive")),
    tag = "system",
)]
async fn health() -> impl IntoResponse {
    Json(ack(true, ""))
}

#[utoipa::path(
    post,
    path = "/api/v1/shutdown",
    responses((status = 200, body = AckBody, description = "shutdown signalled")),
    tag = "system",
)]
async fn shutdown(State(state): State<HttpState>) -> impl IntoResponse {
    tracing::info!("shutdown requested via control plane");
    state.inner.shutdown.cancel();
    Json(ok_ack())
}

#[utoipa::path(
    post,
    path = "/api/v1/agents",
    request_body = SpawnAgentBody,
    responses(
        (status = 200, body = AgentIdBody, description = "spawned agent id"),
        (status = 400, body = String, description = "spawn failed"),
    ),
    tag = "agents",
)]
async fn spawn_agent(
    State(state): State<HttpState>,
    Json(body): Json<SpawnAgentBody>,
) -> Result<Json<AgentIdBody>, (StatusCode, String)> {
    match state.inner.pm.spawn_by_kind(&body.kind, body.opts).await {
        Ok(id) => Ok(Json(AgentIdBody {
            agent_id: id.to_string(),
        })),
        Err(e) => Err(err_status(StatusCode::BAD_REQUEST, e)),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/agents/{id}/start",
    params(("id" = String, Path, description = "agent UUID")),
    responses(
        (status = 200, body = AckBody),
        (status = 400, body = String, description = "invalid agent id"),
    ),
    tag = "agents",
)]
async fn start_agent(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AckBody>, (StatusCode, String)> {
    let id = parse_id(&id)?;
    Ok(Json(to_ack(state.inner.pm.start(&id))))
}

#[utoipa::path(
    post,
    path = "/api/v1/agents/{id}/stop",
    params(("id" = String, Path, description = "agent UUID")),
    responses(
        (status = 200, body = AckBody),
        (status = 400, body = String, description = "invalid agent id"),
    ),
    tag = "agents",
)]
async fn stop_agent(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AckBody>, (StatusCode, String)> {
    let id = parse_id(&id)?;
    Ok(Json(to_ack(state.inner.pm.stop(&id))))
}

#[utoipa::path(
    post,
    path = "/api/v1/agents/{id}/restart",
    params(("id" = String, Path, description = "agent UUID")),
    responses(
        (status = 200, body = AckBody),
        (status = 400, body = String, description = "invalid agent id"),
    ),
    tag = "agents",
)]
async fn restart_agent(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AckBody>, (StatusCode, String)> {
    let id = parse_id(&id)?;
    Ok(Json(to_ack(state.inner.pm.restart(&id))))
}

#[utoipa::path(
    post,
    path = "/api/v1/agents/{id}/messages",
    params(("id" = String, Path, description = "agent UUID")),
    request_body = InjectMessageBody,
    responses(
        (status = 200, body = AckBody),
        (status = 400, body = String, description = "invalid agent id"),
    ),
    tag = "agents",
)]
async fn inject_message(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<InjectMessageBody>,
) -> Result<Json<AckBody>, (StatusCode, String)> {
    let id = parse_id(&id)?;
    Ok(Json(to_ack(state.inner.pm.inject_message(&id, body.content))))
}

#[utoipa::path(
    get,
    path = "/api/v1/agents",
    responses((status = 200, body = Vec<AgentInfoBody>, description = "managed agents")),
    tag = "agents",
)]
async fn list_agents(State(state): State<HttpState>) -> Json<Vec<AgentInfoBody>> {
    let snapshot = state.inner.pm.snapshot().await;
    Json(
        snapshot
            .into_iter()
            .map(|(id, kind, status)| AgentInfoBody {
                agent_id: id.to_string(),
                kind,
                status,
            })
            .collect(),
    )
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
struct AgentInfoBody {
    agent_id: String,
    kind: String,
    status: AgentLifecycleStatus,
}

#[utoipa::path(
    get,
    path = "/api/v1/agents/{id}/status",
    params(("id" = String, Path, description = "agent UUID")),
    responses(
        (status = 200, body = StatusBody),
        (status = 400, body = String, description = "invalid agent id"),
        (status = 404, body = String, description = "agent not found"),
    ),
    tag = "agents",
)]
async fn get_status(
    State(state): State<HttpState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<StatusBody>, (StatusCode, String)> {
    let id = parse_id(&id)?;
    match state.inner.pm.status(&id) {
        Ok(status) => Ok(Json(StatusBody { status })),
        Err(e) => Err(err_status(StatusCode::NOT_FOUND, e)),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/pool/models",
    responses((status = 200, body = Vec<String>, description = "model names in the pool")),
    tag = "pool",
)]
async fn pool_models(State(state): State<HttpState>) -> Json<Vec<String>> {
    Json(state.inner.pm.pool_model_names().await)
}

#[utoipa::path(
    put,
    path = "/api/v1/pool",
    request_body = ModelConfig,
    responses((status = 200, body = AckBody, description = "pool reconfigured")),
    tag = "pool",
)]
async fn configure_pool(
    State(state): State<HttpState>,
    Json(cfg): Json<ModelConfig>,
) -> Result<Json<AckBody>, (StatusCode, String)> {
    let res = state.inner.pm.configure_pool(&cfg).await;
    Ok(Json(to_ack(res)))
}

#[utoipa::path(
    get,
    path = "/api/v1/kinds",
    responses((status = 200, body = Vec<String>, description = "registered agent kind names")),
    tag = "pool",
)]
async fn list_kinds(State(state): State<HttpState>) -> Json<Vec<String>> {
    Json(state.inner.pm.registry().list())
}

#[utoipa::path(
    get,
    path = "/api/v1/events",
    responses((status = 200, content_type = "text/event-stream",
        description = "SSE stream of ProcessEvent frames; each `data:` line is a JSON ProcessEvent")),
    tag = "agents",
)]
async fn stream_events(
    State(state): State<HttpState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.inner.pm.events();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        result.ok().map(|event: ProcessEvent| {
            let data = serde_json::to_string(&event).unwrap_or_default();
            Ok::<_, std::convert::Infallible>(Event::default().data(data))
        })
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ── Helpers ─────────────────────────────────────────────────────

fn parse_id(s: &str) -> Result<uuid::Uuid, (StatusCode, String)> {
    uuid::Uuid::parse_str(s).map_err(|e| err_status(StatusCode::BAD_REQUEST, format!("invalid agent_id '{s}': {e}")))
}

fn to_ack(res: Result<(), crate::process::ProcessError>) -> AckBody {
    match res {
        Ok(()) => ok_ack(),
        Err(e) => ack(false, e.to_string()),
    }
}
