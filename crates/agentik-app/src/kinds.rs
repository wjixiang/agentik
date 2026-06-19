//! Agent kind registrations for the host binary.
//!
//! Each function here returns a fully constructed [`AgentBlueprint`] that bundles
//! a tool provider.

use std::sync::Arc;

use agentik_runtime::registry::AgentBlueprint;

/// A generic "coder" agent kind that uses the built-in core tools
/// (bash, read, write, edit, glob, grep, webfetch).
pub fn coder_kind() -> Arc<AgentBlueprint> {
    let mut tool_provider = agentik_core::tools::ToolProviderRegistry::new();

    // Register all primitive tools from agentik-tools.
    for reg in agentik_tools::primitive_registrations() {
        tool_provider.register(reg);
    }

    Arc::new(
        AgentBlueprint::new(
            "coder",
            "Generic Coder",
            tool_provider,
        )
        .with_identity(
            "You are a helpful coding assistant. You can read, write, and edit files, \
             run shell commands, and fetch web content.",
        ),
    )
}
