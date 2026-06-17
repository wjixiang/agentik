//! Built-in agent kinds registered by the runtime daemon at startup.
//!
//! The `coder` kind's skill tree is sourced from the shared
//! [`SqliteSkillStore`] (the single source of truth), rather than rebuilt from
//! skill directories — so the tree the agent sees as its system prompt matches
//! what the control plane manages.

use std::sync::Arc;

use agentik_core::tools::ToolProviderRegistry;
use agentik_skill::SkillTree;
use agentik_skill_client::SkillRegistryClient;
use agentik_skill_server::SqliteSkillStore;

use tokio::sync::Mutex;

use crate::registry::AgentBlueprint;

/// Build a generic "coder" agent kind from a pre-built skill tree.
///
/// Uses the built-in primitive tools (bash, read, write, edit, glob, grep,
/// webfetch). If `skill_client` is provided, the `activate_skill` tool is
/// registered on agents built from this kind.
pub fn coder_kind_with_tree(
    skill_tree: SkillTree,
    skill_client: Option<Arc<Mutex<SkillRegistryClient>>>,
) -> Arc<AgentBlueprint> {
    let mut tool_provider = ToolProviderRegistry::new();

    // Register all primitive tools from agentik-tools.
    for reg in agentik_tools::primitive_registrations() {
        tool_provider.register(reg);
    }

    let mut blueprint = AgentBlueprint::new(
        "coder",
        "Generic Coder",
        skill_tree,
        tool_provider,
    )
    .with_identity(
        "You are a helpful coding assistant. You can read, write, and edit files, \
         run shell commands, and fetch web content.",
    );

    if let Some(client) = skill_client {
        blueprint = blueprint.with_skill_client(client);
    }

    Arc::new(blueprint)
}

/// Build the `coder` kind with its skill tree sourced from the shared store.
///
/// The store is the single source of truth; this fetches the current tree
/// (with auto-generated children summary on the root) and hands it to
/// [`coder_kind_with_tree`]. Used both at daemon startup and when refreshing
/// the kind after a skill import.
pub async fn coder_kind(
    store: &SqliteSkillStore,
    skill_client: Option<Arc<Mutex<SkillRegistryClient>>>,
) -> Result<Arc<AgentBlueprint>, agentik_skill_server::store::SkillStoreError> {
    let tree = store.skill_tree().await?;
    Ok(coder_kind_with_tree(tree, skill_client))
}
