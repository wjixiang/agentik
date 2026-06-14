pub mod sqlite_storage;

use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;

use crate::{lifecycle::AgentLifecycleStatus, memory::Memory};

#[derive(Debug, Clone)]
pub struct AgentSnapshot {
    pub ts: i64,
    pub agent_id: Uuid,
    pub agent_status: AgentLifecycleStatus,
    pub memory: Memory,
}

#[derive(Debug, Error)]
pub enum AgentSnapshotStorageError {
    #[error("snapshot storage error")]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
pub trait AgentSnapshotStorage: Send + Sync {
    async fn create_snapshot(
        &self,
        snapshot: AgentSnapshot,
    ) -> Result<(), AgentSnapshotStorageError>;
    async fn get_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> Result<AgentSnapshot, AgentSnapshotStorageError>;
    async fn get_agent_snapshots(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<AgentSnapshot>, AgentSnapshotStorageError>;
}
