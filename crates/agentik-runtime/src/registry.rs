//! Agent registry — named agent "kinds" that bundle tools.
//!
//! A **host binary** (which depends on `agentik-core`) constructs an [`AgentBlueprint`]
//! that bundles a tool provider, then registers
//! it via [`AgentRegistry`]. The runtime calls [`AgentBlueprint::build_agent`] internally
//! when spawning or rebuilding an agent.

use std::collections::HashMap;
use std::sync::Arc;

use agentik_core::tools::ToolProviderRegistry;

// ── Error ───────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AgentBlueprintError {
    #[error("agent kind '{0}' not registered")]
    NotFound(String),
    #[error("kind '{kind}' failed to build: {reason}")]
    BuildFailed { kind: String, reason: String },
}

// ── Spawn options (frontend-facing, pure data) ───────────────────

/// Options the frontend supplies when spawning an agent by kind.
///
/// Defined in [`agentik_api`] and re-exported here so historical
/// `agentik_runtime::AgentSpawnOpts` paths keep resolving.
pub use agentik_api::AgentSpawnOpts;

// ── AgentBlueprint ────────────────────────────────────────────────────

/// A named kind of agent that bundles a tool provider.
///
/// This is the unified construction layer that binds Agent + Toolset.
/// When `build_agent()` is called, it builds the toolset from the
/// tool provider and constructs the agent.
///
/// Example:
/// ```ignore
/// let kind = AgentBlueprint::new(
///     "coder",
///     "Generic Coder",
///     default_tool_provider(),
/// )
/// .with_identity("You are a helpful coding assistant.");
///
/// let agent = kind.build_agent(model_pool).await?;
/// ```
pub struct AgentBlueprint {
    pub name: String,
    pub display_name: String,
    pub tool_provider: ToolProviderRegistry,
    pub default_identity: Option<String>,
}

impl AgentBlueprint {
    /// Create a new agent kind.
    ///
    /// # Arguments
    /// * `name` — Machine-readable identifier (e.g. "coder")
    /// * `display_name` — Human-readable label for UI
    /// * `tool_provider` — Global tool provider for resolving tool names to implementations
    pub fn new(
        name: impl Into<String>,
        display_name: impl Into<String>,
        tool_provider: ToolProviderRegistry,
    ) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            tool_provider,
            default_identity: None,
        }
    }

    /// Set a default prompt identity for this kind.
    pub fn with_identity(mut self, identity: impl Into<String>) -> Self {
        self.default_identity = Some(identity.into());
        self
    }

    /// Build a complete Agent from this kind's tool provider.
    ///
    /// The toolset is built from the tool provider (includes lifecycle tools).
    pub async fn build_agent(
        &self,
        model_pool: Arc<agentik_core::model::model_pool::ModelPool>,
    ) -> Result<agentik_core::Agent, AgentBlueprintError> {
        use agentik_core::agent_builder::AgentBuilder;

        // Build toolset from tool provider (includes lifecycle tools)
        let toolset = self.tool_provider.build_toolset(&[], true);

        // Build agent via builder (prebuilt toolset skips auto-registration)
        let mut builder = AgentBuilder::new()
            .with_model_pool(model_pool)
            .with_toolset(toolset);

        if let Some(identity) = &self.default_identity {
            builder = builder.with_system_prompt_identity(identity.clone());
        }

        builder.build().await.map_err(|e| AgentBlueprintError::BuildFailed {
            kind: self.name.clone(),
            reason: e.to_string(),
        })
    }
}

// ── Registry ─────────────────────────────────────────────────────

/// Thread-safe registry of named agent kinds.
///
/// The host registers [`AgentBlueprint`] instances at startup.
/// The runtime looks up kinds by name when the frontend calls
/// [`spawn_by_kind`](crate::AgentManager::spawn_by_kind).
#[derive(Default)]
pub struct AgentRegistry {
    kinds: std::sync::RwLock<HashMap<String, Arc<AgentBlueprint>>>,
}

impl AgentRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an agent kind.  Replaces any existing kind with the same name.
    pub fn register(&self, kind: Arc<AgentBlueprint>) {
        let name = kind.name.clone();
        self.kinds.write().unwrap().insert(name, kind);
    }

    /// Remove a registered kind by name.
    pub fn unregister(&self, name: &str) {
        self.kinds.write().unwrap().remove(name);
    }

    /// List all registered kind names.
    pub fn list(&self) -> Vec<String> {
        self.kinds.read().unwrap().keys().cloned().collect()
    }

    /// Look up a kind by name.
    pub fn get(&self, name: &str) -> Option<Arc<AgentBlueprint>> {
        self.kinds.read().unwrap().get(name).cloned()
    }
}
