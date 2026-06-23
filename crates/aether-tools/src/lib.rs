//! Aether agent tools: Iceberg namespace/table CRUD (REST catalog) and
//! DataFusion-level table discovery, schema inspection, and SQL preview.
//!
//! All tools operate through the shared [`AetherWorkspace`] so that the
//! underlying REST catalog connection and DataFusion session are reused
//! across every invocation.
//!
//! Wire the tools into an agent's toolset via [`aether_registrations`].

pub mod common;
pub mod describe_table;
pub mod list_tables;
pub mod namespace;
pub mod query;
pub mod table;

use std::sync::Arc;

use iceberg::Namespace;
use serde_json::{Value, json};

pub use agentik_core::tools::ToolRegistration;
pub use datalake::aether::AetherWorkspace;
pub use describe_table::{AetherDescribeTableInput, AetherDescribeTableTool};
pub use list_tables::{AetherListTablesInput, AetherListTablesTool};
pub use namespace::{
    AetherCreateNamespaceInput, AetherCreateNamespaceTool, AetherDropNamespaceInput,
    AetherDropNamespaceTool, AetherListNamespacesInput, AetherListNamespacesTool,
    AetherNamespaceExistsInput, AetherNamespaceExistsTool,
};
pub use query::{AetherPreviewTableInput, AetherPreviewTableTool};
pub use table::{
    AetherCreateTableInput, AetherCreateTableTool, AetherDropTableInput, AetherDropTableTool,
    AetherListTablesInNamespaceInput, AetherListTablesInNamespaceTool, AetherLoadTableInput,
    AetherLoadTableTool, AetherRenameTableInput, AetherRenameTableTool, AetherTableExistsInput,
    AetherTableExistsTool,
};

/// Serialize an Iceberg [`Namespace`] into a JSON object.
fn ns_to_json(ns: &Namespace, already_exists: bool) -> Value {
    json!({
        "namespace": ns.name().as_ref().join("."),
        "properties": ns.properties(),
        "already_exists": already_exists,
    })
}

/// All aether tool registrations, ready to register into a toolset.
///
/// Returns 13 tools:
/// - **DataFusion** (2): `aether_list_tables`, `aether_describe_table`
/// - **Namespace CRUD** (4): `aether_list_namespaces`, `aether_create_namespace`,
///   `aether_namespace_exists`, `aether_drop_namespace`
/// - **Table CRUD** (6): `aether_list_tables_in_namespace`, `aether_table_exists`,
///   `aether_load_table`, `aether_create_table`, `aether_drop_table`,
///   `aether_rename_table`
/// - **SQL preview** (1): `aether_preview_table`
pub fn aether_registrations(workspace: Arc<AetherWorkspace>) -> Vec<ToolRegistration> {
    vec![
        // DataFusion-level tools
        ToolRegistration::from(AetherListTablesTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherDescribeTableTool {
            workspace: workspace.clone(),
        }),
        // namespace tools
        ToolRegistration::from(AetherListNamespacesTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherCreateNamespaceTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherNamespaceExistsTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherDropNamespaceTool {
            workspace: workspace.clone(),
        }),
        // table tools
        ToolRegistration::from(AetherListTablesInNamespaceTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherTableExistsTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherLoadTableTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherCreateTableTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherDropTableTool {
            workspace: workspace.clone(),
        }),
        ToolRegistration::from(AetherRenameTableTool {
            workspace: workspace.clone(),
        }),
        // read tool
        ToolRegistration::from(AetherPreviewTableTool { workspace }),
    ]
}
