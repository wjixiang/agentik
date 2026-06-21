//! Data lake endpoints: namespace browsing, table listing, SQL queries.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    routing::get,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

/// GET /api/datalake/namespaces — list all namespaces.
async fn list_namespaces(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<String>>, AppError> {
    let namespaces = crate::services::datalake_service::list_namespaces(&state).await?;
    Ok(Json(namespaces))
}

/// GET /api/datalake/namespaces/:ns/tables — list tables in a namespace.
async fn list_tables(
    State(state): State<Arc<AppState>>,
    Path(ns): Path<String>,
) -> Result<Json<Vec<TableInfo>>, AppError> {
    let tables = crate::services::datalake_service::list_tables(&state, &ns).await?;
    Ok(Json(tables))
}

/// GET /api/datalake/query?sql=... — execute a SQL query.
async fn query(
    State(state): State<Arc<AppState>>,
    Query(params): Query<QueryParams>,
) -> Result<Json<QueryResult>, AppError> {
    let result = crate::services::datalake_service::query(&state, &params.sql).await?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub namespace: String,
}

#[derive(Debug, Serialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: usize,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/datalake/namespaces", get(list_namespaces))
        .route("/api/datalake/namespaces/{ns}/tables", get(list_tables))
        .route("/api/datalake/query", get(query))
}
