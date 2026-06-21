//! Agent pool management: spawn, track, and bridge events to the global bus.

use std::sync::Arc;

use tokio::sync::mpsc;
use uuid::Uuid;

use agentik_core::Agent;
use agentik_sdk::types::{AgentEvent, ContentBlock};
use agentik_tools::primitive_registrations;

use crate::state::AppState;

/// A running agent instance tracked by the pool.
pub struct AgentHandle {
    pub id: Uuid,
    pub identity: String,
    /// The agent itself. Wrapped in Mutex for interior mutability.
    agent: Arc<tokio::sync::Mutex<Agent>>,
}

impl AgentHandle {
    /// Get a status string for the agent.
    pub fn status(&self) -> &'static str {
        "running"
    }

    /// Inject a user message and start the agent loop.
    pub async fn send_message(&self, content: &str) -> anyhow::Result<()> {
        let agent = self.agent.clone();
        let mut guard = agent.lock().await;
        let user_content = vec![ContentBlock::Text {
            text: content.to_string(),
        }];
        guard.inject_message(user_content)?;

        // Start the agent loop in background (spawn a task that holds the lock)
        let agent_clone = Arc::clone(&self.agent);
        let id = self.id;
        tokio::spawn(async move {
            let mut a = agent_clone.lock().await;
            if let Err(e) = a.start().await {
                tracing::error!("Agent {} error: {}", id, e);
            }
        });

        Ok(())
    }
}

/// Get an existing agent or create a new one.
pub async fn get_or_create_agent(
    state: &AppState,
    id: Option<Uuid>,
    identity: Option<&str>,
) -> anyhow::Result<Arc<AgentHandle>> {
    let agent_id = id.unwrap_or_else(Uuid::new_v4);

    // Check if agent already exists
    {
        let agents = state.agents.read().await;
        if let Some(existing) = agents.get(&agent_id) {
            return Ok(Arc::new(AgentHandle {
                id: existing.id,
                identity: existing.identity.clone(),
                agent: existing.agent.clone(),
            }));
        }
    }

    // Create a new agent
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    let tools = primitive_registrations();
    let mut agent = Agent::builder()
        .with_event_tx(event_tx)
        .with_tools(tools)
        .with_system_prompt_identity(
            identity.unwrap_or("You are a helpful AI assistant."),
        )
        .build()
        .await?;

    let handle = Arc::new(AgentHandle {
        id: agent.id(),
        identity: identity.unwrap_or("default").to_string(),
        agent: Arc::new(tokio::sync::Mutex::new(agent)),
    });

    // Spawn bridge task: forward agent events to global broadcast bus
    let broker_tx = state.event_broker.clone();
    let bridge_agent_id = handle.id;
    tokio::spawn(async move {
        bridge_agent_events(bridge_agent_id, event_rx, broker_tx).await;
    });

    state.agents.write().await.insert(handle.id, AgentHandle {
        id: handle.id,
        identity: handle.identity.clone(),
        agent: handle.agent.clone(),
    });

    Ok(handle)
}

/// Remove an agent from the pool.
pub async fn remove_agent(state: &AppState, agent_id: Uuid) -> anyhow::Result<()> {
    state.agents.write().await.remove(&agent_id);
    Ok(())
}

/// Bridge task: reads from a single agent's mpsc channel and forwards
/// to the global broadcast bus tagged with the agent's UUID.
async fn bridge_agent_events(
    agent_id: Uuid,
    mut rx: mpsc::UnboundedReceiver<AgentEvent>,
    tx: tokio::sync::broadcast::Sender<(Uuid, AgentEvent)>,
) {
    while let Some(event) = rx.recv().await {
        // Ignore send errors (no subscribers or lagging)
        let _ = tx.send((agent_id, event));
    }
}
