//! Phloem Server — Web backend for the agentik agent framework.
//!
//! Named after phloem, the vascular tissue that transports nutrients
//! and signals between organs in plants — this server transports data,
//! events, and configuration between the agent runtime and the web UI.

mod config;
mod error;
mod middleware;
mod routes;
mod services;
mod sse;
mod state;

use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "phloem_server=debug,tower_http=debug".into()),
        )
        .init();

    let config = config::ServerConfig::from_env();
    let state = AppState::new(config.clone());

    let app = Router::new()
        .merge(routes::create_router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state));

    let addr = format!("{}:{}", config.host, config.port);
    info!("Phloem server starting on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
