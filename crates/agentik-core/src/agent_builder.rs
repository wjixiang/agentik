use std::sync::Arc;

use agentik_sdk::model::model_pool::ModelPool;
use uuid::Uuid;

use crate::agent::{Agent, AgentConfig, TokenBudget};
use crate::context::ContextProvider;
use crate::error::AgentError;
use agentik_sdk::types::messages::Message;
use crate::storage::AgentSnapshotStorage;
use crate::{lifecycle::AgentLifecycle, memory::Memory, tools::Toolset};
use crate::tools::ToolRegistration;

pub struct AgentBuilder {
    model_pool: Option<Arc<ModelPool>>,
    initial_messages: Vec<Message>,
    context_provider: Option<Arc<dyn ContextProvider>>,
    config: AgentConfig,
    storage: Option<Arc<dyn AgentSnapshotStorage>>,
    tools: Vec<ToolRegistration>,
    system_prompt_section: Option<String>,
    system_prompt_identity: Option<String>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<agentik_sdk::types::AgentUiEvent>>,
    /// Pre-built toolset — when set, skips automatic tool registration.
    prebuilt_toolset: Option<Toolset>,
}

impl Clone for AgentBuilder {
    fn clone(&self) -> Self {
        Self {
            model_pool: self.model_pool.clone(),
            initial_messages: self.initial_messages.clone(),
            context_provider: self.context_provider.clone(),
            config: self.config.clone(),
            storage: self.storage.clone(),
            tools: Vec::new(), // ToolRegistration is not Clone; re-register if needed
            system_prompt_section: self.system_prompt_section.clone(),
            system_prompt_identity: self.system_prompt_identity.clone(),
            event_tx: self.event_tx.clone(),
            prebuilt_toolset: None,
        }
    }
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            model_pool: None,
            initial_messages: Vec::new(),
            context_provider: None,
            config: AgentConfig::default(),
            storage: None,
            tools: Vec::new(),
            system_prompt_section: None,
            system_prompt_identity: None,
            event_tx: None,
            prebuilt_toolset: None,
        }
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_model_pool(mut self, pool: Arc<ModelPool>) -> Self {
        self.model_pool = Some(pool);
        self
    }

    /// Set initial messages to seed the agent's memory at build time.
    pub fn with_initial_messages(mut self, messages: Vec<Message>) -> Self {
        self.initial_messages = messages;
        self
    }

    /// Set an optional context provider for dynamic context injection.
    pub fn with_context_provider(mut self, provider: Arc<dyn ContextProvider>) -> Self {
        self.context_provider = Some(provider);
        self
    }

    pub fn with_storage(mut self, storage: Arc<dyn AgentSnapshotStorage>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Register additional tools on the agent (beyond the built-in lifecycle tools).
    pub fn with_tools(mut self, tools: Vec<ToolRegistration>) -> Self {
        self.tools = tools;
        self
    }

    /// Set a static extra section for the system prompt.
    pub fn with_system_prompt_section(mut self, section: impl Into<String>) -> Self {
        self.system_prompt_section = Some(section.into());
        self
    }

    /// Set the agent identity line for the system prompt (e.g. "You are a biomedical research assistant.").
    pub fn with_system_prompt_identity(mut self, identity: impl Into<String>) -> Self {
        self.system_prompt_identity = Some(identity.into());
        self
    }

    /// Wire an event channel for streaming `AgentUiEvent`s to external observers (e.g. a TUI).
    pub fn with_event_tx(
        mut self,
        tx: tokio::sync::mpsc::UnboundedSender<agentik_sdk::types::AgentUiEvent>,
    ) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Provide a pre-built toolset. When set, the builder skips automatic
    /// tool registration (lifecycle + `with_tools`) and uses this toolset
    /// directly.
    pub fn with_toolset(mut self, toolset: Toolset) -> Self {
        self.prebuilt_toolset = Some(toolset);
        self
    }

    pub async fn build(self) -> Result<Agent, AgentError> {
        let model_pool = self
            .model_pool
            .ok_or_else(|| AgentError::MissingConfig("model_pool".to_string()))?;

        // Build toolset: use prebuilt if provided, otherwise auto-register.
        let toolset = if let Some(toolset) = self.prebuilt_toolset {
            toolset
        } else {
            let mut toolset = Toolset::default();
            toolset.register_all(crate::tools::lifecycle_registrations())?;
            toolset.register_all(self.tools)?;
            toolset
        };

        // Seed memory with initial messages
        let mut memory = Memory::new();
        for msg in self.initial_messages {
            let _ = memory.remember(msg);
        }

        Ok(Agent {
            id: Uuid::new_v4(),
            model_pool,
            memory,
            toolset,
            lifecycle: AgentLifecycle::new(),
            config: self.config,
            storage: self.storage,
            token_budget: TokenBudget::default(),
            context_provider: self.context_provider,
            system_prompt_section: self.system_prompt_section,
            system_prompt_identity: self.system_prompt_identity,
            event_tx: self.event_tx,
            current_model_name: None,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
