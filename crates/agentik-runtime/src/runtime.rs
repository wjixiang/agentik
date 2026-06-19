//! Unified runtime portal that owns the process manager.
//!
//! [`Runtime`] is the single entry point for the host binary. It:
//! 1. Owns a [`AgentManager`] (which in turn owns the agent registry and model pool).
//! 2. Provides graceful shutdown of all agents.

use std::sync::Arc;

use thiserror::Error;

use crate::pool::PoolOwner;
use crate::process::{ProcessEvent, ProcessExitStatus, AgentManager};
use crate::registry::AgentRegistry;

// ── Error ───────────────────────────────────────────────────────

/// Errors produced by [`Runtime`] initialization or operation.
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("process manager error: {0}")]
    Process(#[from] crate::process::ProcessError),
}

// ── Configuration ───────────────────────────────────────────────

/// Declarative configuration for constructing a [`Runtime`].
#[derive(Default, Clone, Debug)]
pub struct RuntimeConfig {
    /// Initial model configuration for the pool.
    /// Can be `None` to defer pool configuration (call
    /// `AgentManager::configure_pool` later).
    pub model_config: Option<crate::ModelConfig>,
}

impl RuntimeConfig {
    /// Create a config with no skill server (headless mode).
    pub fn headless() -> Self {
        Self::default()
    }

    /// Set the initial model configuration.
    pub fn with_model_config(mut self, config: crate::ModelConfig) -> Self {
        self.model_config = Some(config);
        self
    }
}

// ── Runtime ────────────────────────────────────────────────────

/// The unified runtime portal for the agent system.
///
/// Owns the agent process manager.
/// Construct via [`Runtime::new`] which handles the full initialization
/// sequence (configure pool).
pub struct Runtime {
    /// The multi-agent process manager.
    process_manager: AgentManager,
}

impl Runtime {
    /// Build and start a [`Runtime`] from the given configuration.
    ///
    /// Initialization sequence:
    /// 1. Create the `AgentManager` and configure the pool (if provided).
    pub async fn new(config: RuntimeConfig) -> Result<Self, RuntimeError> {
        let registry = Arc::new(AgentRegistry::new());
        let pool = Arc::new(PoolOwner::new());

        // ── Process manager ──
        let process_manager = AgentManager::with_registry_and_pool(registry, pool);

        // Configure pool if provided.
        if let Some(ref model_config) = config.model_config {
            process_manager
                .configure_pool(model_config)
                .await
                .map_err(RuntimeError::Process)?;
        }

        Ok(Self { process_manager })
    }

    // ── Accessors ─────────────────────────────────────────────

    /// Access the process manager for lifecycle control (spawn, start, stop, etc.).
    pub fn process_manager(&self) -> &AgentManager {
        &self.process_manager
    }

    /// Access the agent registry (for registering agent kinds).
    pub fn registry(&self) -> &AgentRegistry {
        self.process_manager.registry()
    }

    /// Subscribe to the aggregated event stream for all agents.
    pub fn events(&self) -> tokio::sync::broadcast::Receiver<ProcessEvent> {
        self.process_manager.events()
    }

    /// Configure the model pool. See [`AgentManager::configure_pool`].
    pub async fn configure_pool(
        &self,
        cfg: &crate::ModelConfig,
    ) -> Result<(), RuntimeError> {
        self.process_manager
            .configure_pool(cfg)
            .await
            .map_err(RuntimeError::Process)?;
        Ok(())
    }

    /// Reconfigure the pool and rebuild all running agents.
    pub async fn reconfigure_pool(&self, cfg: &crate::ModelConfig) -> Result<usize, RuntimeError> {
        self.process_manager
            .reconfigure_pool(cfg)
            .await
            .map_err(RuntimeError::Process)
    }

    // ── Shutdown ────────────────────────────────────────────

    /// Gracefully shut down the runtime.
    ///
    /// Shuts down all agents (cancel + await their tasks).
    ///
    /// Returns the exit statuses of all agents.
    pub async fn shutdown(self) -> Vec<(uuid::Uuid, ProcessExitStatus)> {
        self.process_manager.shutdown().await
    }
}
