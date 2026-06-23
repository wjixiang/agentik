//! Table-level Iceberg tools: list (REST, nested-ns capable), inspect metadata,
//! create, drop, rename, and check existence.
//!
//! All tools route through the shared [`AetherWorkspace`] catalog.

use std::collections::HashMap;
use std::sync::Arc;

use agentik_core::tools::{ToolError, ToolFunction};
use agentik_sdk::types::ToolResult;
use async_trait::async_trait;
use datalake::aether::AetherWorkspace;
use iceberg::{Catalog, TableCreation};
use serde::{Deserialize, Serialize};

use crate::common::{build_schema, err, parse_columns, table_ident};

// --- list tables (REST catalog, supports nested namespaces) -----------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_list_tables_in_namespace",
    description = "List Iceberg tables within a namespace via the REST catalog. Supports nested (multi-segment) namespace paths, e.g. 'warehouse.analytics'. Returns table names (not their full identifiers). For top-level namespaces the DataFusion-based aether_list_tables can also be used."
)]
pub struct AetherListTablesInNamespaceInput {
    #[desc = "Namespace path (dotted) to list tables in, e.g. 'warehouse.analytics'"]
    pub namespace: String,
}

pub struct AetherListTablesInNamespaceTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherListTablesInNamespaceTool {
    type Input = AetherListTablesInNamespaceInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let namespace = crate::common::parse_namespace(&input.namespace).map_err(err)?;

        let tables = catalog.list_tables(&namespace).await.map_err(err)?;
        let names: Vec<String> = tables.into_iter().map(|t| t.name).collect();

        Ok(ToolResult::success_json(serde_json::json!({
            "namespace": input.namespace,
            "tables": names,
            "count": names.len(),
        })))
    }
}

// --- table exists -----------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_table_exists",
    description = "Check whether an Iceberg table exists. Returns a JSON object with an `exists` boolean."
)]
pub struct AetherTableExistsInput {
    #[desc = "Namespace path (dotted) containing the table, e.g. 'warehouse.analytics'"]
    pub namespace: String,
    #[desc = "Table name"]
    pub table: String,
}

pub struct AetherTableExistsTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherTableExistsTool {
    type Input = AetherTableExistsInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let ident = table_ident(&input.namespace, &input.table).map_err(err)?;
        let exists = catalog.table_exists(&ident).await.map_err(err)?;
        Ok(ToolResult::success_json(serde_json::json!({
            "table": format!("{}.{}", input.namespace, input.table),
            "exists": exists,
        })))
    }
}

// --- load table (metadata) --------------------------------------------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_load_table",
    description = "Load an Iceberg table's metadata: schema columns, location, format version, properties, and the current snapshot. Does not read row data — use aether_preview_table for that."
)]
pub struct AetherLoadTableInput {
    #[desc = "Namespace path (dotted) containing the table, e.g. 'warehouse.analytics'"]
    pub namespace: String,
    #[desc = "Table name"]
    pub table: String,
}

pub struct AetherLoadTableTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherLoadTableTool {
    type Input = AetherLoadTableInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let ident = table_ident(&input.namespace, &input.table).map_err(err)?;
        let table = catalog.load_table(&ident).await.map_err(err)?;
        let metadata = table.metadata();

        let columns: Vec<serde_json::Value> = metadata
            .current_schema()
            .as_struct()
            .fields()
            .iter()
            .map(|f| {
                serde_json::json!({
                    "id": f.id,
                    "name": f.name,
                    "type": f.field_type.to_string(),
                    "required": f.required,
                })
            })
            .collect();

        let snapshot = metadata.current_snapshot().map(|s| {
            serde_json::json!({
                "snapshot_id": s.snapshot_id(),
                "timestamp_ms": s.timestamp_ms(),
                "parent_snapshot_id": s.parent_snapshot_id(),
            })
        });

        Ok(ToolResult::success_json(serde_json::json!({
            "identifier": format!("{}", ident),
            "location": metadata.location(),
            "format_version": format!("{:?}", metadata.format_version()),
            "schema_id": metadata.current_schema_id(),
            "columns": columns,
            "properties": metadata.properties(),
            "current_snapshot": snapshot,
            "metadata_location": table.metadata_location(),
        })))
    }
}

// --- create table -----------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_create_table",
    description = "Create an Iceberg table with a simple schema. Columns are specified as compact 'name:type' strings (append '!' to mark required), e.g. ['id:long!', 'event:string', 'ts:timestamp']. Supported types: boolean, int, long, float, double, date, time, timestamp, timestamptz, string, uuid, binary."
)]
pub struct AetherCreateTableInput {
    #[desc = "Namespace path (dotted) to create the table in, e.g. 'warehouse.analytics'"]
    pub namespace: String,
    #[desc = "Table name to create"]
    pub table: String,
    #[desc = "Column specs as 'name:type' (or 'name:type!' for required), e.g. ['id:long!', 'event:string']"]
    pub columns: Vec<String>,
    #[desc = "Optional table location. Defaults to the catalog/warehouse default."]
    pub location: Option<String>,
    #[desc = "If true, succeed by loading the existing table when it already exists. Defaults to false."]
    pub if_not_exists: Option<bool>,
}

pub struct AetherCreateTableTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherCreateTableTool {
    type Input = AetherCreateTableInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let namespace = crate::common::parse_namespace(&input.namespace).map_err(err)?;
        let ident = table_ident(&input.namespace, &input.table).map_err(err)?;

        if input.if_not_exists.unwrap_or(false) && catalog.table_exists(&ident).await.map_err(err)? {
            let table = catalog.load_table(&ident).await.map_err(err)?;
            return Ok(ToolResult::success_json(serde_json::json!({
                "identifier": format!("{}", ident),
                "created": false,
                "location": table.metadata().location(),
            })));
        }

        let columns = parse_columns(&input.columns).map_err(err)?;
        let schema = build_schema(&columns).map_err(err)?;

        let location_opt = input
            .location
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let creation = TableCreation::builder()
            .name(input.table.clone())
            .schema(schema)
            .properties(HashMap::new())
            .location_opt(location_opt)
            .build();

        let table = catalog
            .create_table(&namespace, creation)
            .await
            .map_err(err)?;

        Ok(ToolResult::success_json(serde_json::json!({
            "identifier": format!("{}", table.identifier()),
            "created": true,
            "location": table.metadata().location(),
        })))
    }
}

// --- drop table -------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_drop_table",
    description = "Drop (delete) an Iceberg table and its metadata. This removes the table from the catalog; underlying data files may be retained depending on the catalog/storage configuration."
)]
pub struct AetherDropTableInput {
    #[desc = "Namespace path (dotted) containing the table, e.g. 'warehouse.analytics'"]
    pub namespace: String,
    #[desc = "Table name to drop"]
    pub table: String,
}

pub struct AetherDropTableTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherDropTableTool {
    type Input = AetherDropTableInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let ident = table_ident(&input.namespace, &input.table).map_err(err)?;
        catalog.drop_table(&ident).await.map_err(err)?;
        Ok(ToolResult::success_json(serde_json::json!({
            "table": format!("{}", ident),
            "dropped": true,
        })))
    }
}

// --- rename table -----------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, agentik_proc::ToolInput)]
#[tool(
    name = "aether_rename_table",
    description = "Rename (move) an Iceberg table to a new namespace and/or name. Both source and destination namespaces must already exist."
)]
pub struct AetherRenameTableInput {
    #[desc = "Source namespace path (dotted), e.g. 'warehouse.analytics'"]
    pub from_namespace: String,
    #[desc = "Source table name"]
    pub from_table: String,
    #[desc = "Destination namespace path (dotted), e.g. 'warehouse.archive'"]
    pub to_namespace: String,
    #[desc = "Destination table name"]
    pub to_table: String,
}

pub struct AetherRenameTableTool {
    pub workspace: Arc<AetherWorkspace>,
}

#[async_trait]
impl ToolFunction for AetherRenameTableTool {
    type Input = AetherRenameTableInput;

    async fn run(&self, input: Self::Input) -> Result<ToolResult, ToolError> {
        let catalog = self.workspace.catalog().await.map_err(err)?;
        let src = table_ident(&input.from_namespace, &input.from_table).map_err(err)?;
        let dst = table_ident(&input.to_namespace, &input.to_table).map_err(err)?;
        catalog.rename_table(&src, &dst).await.map_err(err)?;
        Ok(ToolResult::success_json(serde_json::json!({
            "renamed": true,
            "from": format!("{}", src),
            "to": format!("{}", dst),
        })))
    }
}
