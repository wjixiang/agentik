//! Read-side Iceberg tool: preview table rows via DataFusion SQL.
//!
//! Note: the DataFusion catalog provider registers only top-level Iceberg
//! namespaces as schemas, so `namespace` here should be a single top-level
//! segment (e.g. `analytics`, not `warehouse.analytics`).

use agentik_core::tools::{ToolError, ToolFunction};
use crate::common::err;
use agentik_sdk::types::ToolResult;
use arrow::util::pretty::pretty_format_batches;
use async_trait::async_trait;
use datalake::datalake::Datalake;
use serde::{Deserialize, Serialize};

/// Quote a SQL identifier with double quotes, escaping embedded quotes.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "iceberg_preview_table",
    description = "Preview rows from an Iceberg table by running a read-only DataFusion SQL query. Pass optional `columns` to project specific columns, a `where_clause` to filter, and a `limit` to cap the number of rows. The namespace must be a single top-level segment (e.g. 'analytics')."
)]
pub struct IcebergPreviewTableInput {
    #[desc = "Top-level namespace segment containing the table, e.g. 'analytics'"]
    pub namespace: String,
    #[desc = "Table name to preview"]
    pub table: String,
    #[desc = "Optional list of columns to project. Defaults to all columns (*)"]
    pub columns: Option<Vec<String>>,
    #[desc = "Optional SQL WHERE clause (without the 'WHERE' keyword), e.g. \"event = 'login'\""]
    pub where_clause: Option<String>,
    #[desc = "Maximum number of rows to return. Defaults to 100."]
    pub limit: Option<usize>,
}

pub struct IcebergPreviewTableTool;

#[async_trait]
impl ToolFunction for IcebergPreviewTableTool {
    type Input = IcebergPreviewTableInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let namespace = input.namespace.trim();
        let table = input.table.trim();
        if namespace.is_empty() || table.is_empty() {
            return Ok(ToolResult::error(
                "both 'namespace' and 'table' are required",
            ));
        }

        let projection = match input.columns.as_ref() {
            Some(cols) if !cols.is_empty() => cols
                .iter()
                .map(|c| quote_ident(c.trim()))
                .collect::<Vec<_>>()
                .join(", "),
            _ => "*".to_string(),
        };

        let mut sql = format!(
            "SELECT {projection} FROM {}.{},{}",
            quote_ident("iceberg"),
            quote_ident(namespace),
            quote_ident(table),
        );
        if let Some(clause) = input
            .where_clause
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            sql.push_str(" WHERE ");
            sql.push_str(clause);
        }
        let limit = input.limit.unwrap_or(100);
        sql.push_str(&format!(" LIMIT {limit}"));

        let ctx = Datalake::default().get_ctx().await.map_err(err)?;

        let batches = ctx
            .sql(&sql)
            .await
            .map_err(err)?
            .collect()
            .await
            .map_err(err)?;

        let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
        let pretty = pretty_format_batches(&batches)
            .map_err(err)?
            .to_string();

        Ok(ToolResult::success_json(serde_json::json!({
            "query": sql,
            "row_count": row_count,
            "truncated": row_count >= limit,
            "result": pretty,
        })))
    }
}
