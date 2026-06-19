//! Built-in agent kinds registered by the runtime daemon at startup.

use std::sync::Arc;

use agentik_core::tools::ToolProviderRegistry;

use crate::registry::AgentBlueprint;

/// Build a generic "coder" agent kind.
///
/// Uses the built-in primitive tools (bash, read, write, edit, glob, grep,
/// webfetch).
pub fn coder_kind() -> Arc<AgentBlueprint> {
    let mut tool_provider = ToolProviderRegistry::new();

    // Register all primitive tools from agentik-tools.
    for reg in agentik_tools::primitive_registrations() {
        tool_provider.register(reg);
    }

    let blueprint = AgentBlueprint::new(
        "coder",
        "Generic Coder",
        tool_provider,
    )
    .with_identity(
        "You are a helpful coding assistant. You can read, write, and edit files, \
         run shell commands, and fetch web content.",
    );

    Arc::new(blueprint)
}
