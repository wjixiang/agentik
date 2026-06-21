//! Data lake service: wraps the `datalake` crate for REST exposure.

use crate::routes::lake::{QueryResult, TableInfo};
use crate::state::AppState;

/// List all Iceberg namespaces.
pub async fn list_namespaces(_state: &AppState) -> anyhow::Result<Vec<String>> {
    // TODO: initialize Datalake once and store in AppState
    // For now, return placeholder
    Ok(vec!["warehouse".to_string()])
}

/// List tables within a namespace.
pub async fn list_tables(_state: &AppState, _namespace: &str) -> anyhow::Result<Vec<TableInfo>> {
    // TODO: call datalake.list_tables_in_namespace(namespace)
    Ok(vec![])
}

/// Execute a SQL query via DataFusion.
pub async fn query(_state: &AppState, _sql: &str) -> anyhow::Result<QueryResult> {
    // TODO: call datalake.get_ctx().sql(sql)
    Ok(QueryResult {
        columns: vec![],
        rows: vec![],
        row_count: 0,
    })
}
