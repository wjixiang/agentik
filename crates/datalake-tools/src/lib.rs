//! Iceberg operation primitive tools for the agentik-core runtime.
//!
//! This crate exposes a basic set of catalog-level Iceberg operations as
//! agent tools: namespace management, table management, and a read-only
//! table preview backed by DataFusion. They build on the [`datalake`]
//! crate, which owns the catalog/DataFusion session wiring.
//!
//! Wire the tools into an agent's toolset via [`iceberg_registrations`].

pub mod common;
pub mod namespace;
pub mod query;
pub mod table;

use iceberg::Namespace;
use serde_json::{Value, json};

pub use agentik_core::tools::ToolRegistration;
pub use namespace::{
    IcebergCreateNamespaceInput, IcebergCreateNamespaceTool, IcebergDropNamespaceInput,
    IcebergDropNamespaceTool, IcebergListNamespacesInput, IcebergListNamespacesTool,
    IcebergNamespaceExistsInput, IcebergNamespaceExistsTool,
};
pub use query::{IcebergPreviewTableInput, IcebergPreviewTableTool};
pub use table::{
    IcebergCreateTableInput, IcebergCreateTableTool, IcebergDropTableInput, IcebergDropTableTool,
    IcebergListTablesInput, IcebergListTablesTool, IcebergLoadTableInput, IcebergLoadTableTool,
    IcebergRenameTableInput, IcebergRenameTableTool, IcebergTableExistsInput,
    IcebergTableExistsTool,
};

/// Serialize an Iceberg [`Namespace`] into a JSON object.
fn ns_to_json(ns: &Namespace, already_exists: bool) -> Value {
    json!({
        "namespace": ns.name().as_ref().join("."),
        "properties": ns.properties(),
        "already_exists": already_exists,
    })
}

/// All Iceberg primitive tool registrations, ready to register into a toolset.
pub fn iceberg_registrations() -> Vec<ToolRegistration> {
    vec![
        // namespaces
        ToolRegistration::from(IcebergListNamespacesTool),
        ToolRegistration::from(IcebergCreateNamespaceTool),
        ToolRegistration::from(IcebergNamespaceExistsTool),
        ToolRegistration::from(IcebergDropNamespaceTool),
        // tables
        ToolRegistration::from(IcebergListTablesTool),
        ToolRegistration::from(IcebergTableExistsTool),
        ToolRegistration::from(IcebergLoadTableTool),
        ToolRegistration::from(IcebergCreateTableTool),
        ToolRegistration::from(IcebergDropTableTool),
        ToolRegistration::from(IcebergRenameTableTool),
        // read
        ToolRegistration::from(IcebergPreviewTableTool),
    ]
}
