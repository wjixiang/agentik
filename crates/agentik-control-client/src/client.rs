use std::str::FromStr;

use tonic::transport::Endpoint;

use agentik_api::{
    AgentLifecycleStatus, AgentSpawnOpts, ContentBlock, ModelConfig, ProcessEvent,
    SkillChangeNotificationWire, SkillTreeNodeWire, SkillWire,
};
use agentik_control_proto::control_plane::{
    agent_control_service_client::AgentControlServiceClient, AgentIdRequest, ConfigurePoolRequest,
    Empty, InjectMessageRequest, SpawnAgentRequest,
};

#[derive(Debug, thiserror::Error)]
pub enum ControlClientError {
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("gRPC connection error: {0}")]
    Connection(#[from] tonic::transport::Error),
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),
    #[error("daemon discovery file not found — is the daemon running?")]
    NoDaemon,
    #[error("payload (de)serialisation error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("server returned an error: {0}")]
    Server(String),
    #[error("unexpected empty response from server")]
    Empty,
}

/// Info about one managed agent, returned by [`ControlClient::list_agents`].
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub agent_id: uuid::Uuid,
    pub kind: String,
    pub status: AgentLifecycleStatus,
}

/// gRPC client for the agentik control plane.
pub struct ControlClient {
    inner: AgentControlServiceClient<tonic::transport::Channel>,
}

impl ControlClient {
    /// Connect to a daemon at the given endpoint (e.g. `"http://127.0.0.1:54321"`).
    pub async fn connect(addr: &str) -> Result<Self, ControlClientError> {
        let endpoint = Endpoint::from_str(addr)
            .map_err(|e| ControlClientError::InvalidAddress(e.to_string()))?;
        let channel = endpoint.connect().await?;
        Ok(Self {
            inner: AgentControlServiceClient::new(channel),
        })
    }

    /// Spawn an agent by registered kind. Returns the new agent's id.
    pub async fn spawn_agent(
        &mut self,
        kind: &str,
        opts: &AgentSpawnOpts,
    ) -> Result<uuid::Uuid, ControlClientError> {
        use agentik_control_proto::control_plane::spawn_agent_response::Result as SpawnResult;
        let opts_json = serde_json::to_string(opts)?;
        let resp = self
            .inner
            .spawn_agent(SpawnAgentRequest {
                kind: kind.to_string(),
                opts_json,
            })
            .await?
            .into_inner();
        match resp.result {
            Some(SpawnResult::AgentId(id)) => {
                uuid::Uuid::parse_str(&id).map_err(|e| {
                    ControlClientError::Server(format!("invalid agent_id from server: {e}"))
                })
            }
            Some(SpawnResult::Error(msg)) => Err(ControlClientError::Server(msg)),
            None => Err(ControlClientError::Empty),
        }
    }

    pub async fn start_agent(&mut self, agent_id: uuid::Uuid) -> Result<(), ControlClientError> {
        let ack = self
            .inner
            .start_agent(AgentIdRequest {
                agent_id: agent_id.to_string(),
            })
            .await?
            .into_inner();
        check_ack(ack)
    }

    pub async fn stop_agent(&mut self, agent_id: uuid::Uuid) -> Result<(), ControlClientError> {
        let ack = self
            .inner
            .stop_agent(AgentIdRequest {
                agent_id: agent_id.to_string(),
            })
            .await?
            .into_inner();
        check_ack(ack)
    }

    pub async fn restart_agent(&mut self, agent_id: uuid::Uuid) -> Result<(), ControlClientError> {
        let ack = self
            .inner
            .restart_agent(AgentIdRequest {
                agent_id: agent_id.to_string(),
            })
            .await?
            .into_inner();
        check_ack(ack)
    }

    pub async fn inject_message(
        &mut self,
        agent_id: uuid::Uuid,
        content: Vec<ContentBlock>,
    ) -> Result<(), ControlClientError> {
        let content_json = serde_json::to_string(&content)?;
        let ack = self
            .inner
            .inject_message(InjectMessageRequest {
                agent_id: agent_id.to_string(),
                content_json,
            })
            .await?
            .into_inner();
        check_ack(ack)
    }

    pub async fn list_agents(&mut self) -> Result<Vec<AgentInfo>, ControlClientError> {
        let resp = self.inner.list_agents(Empty {}).await?.into_inner();
        resp.agents
            .into_iter()
            .map(|a| {
                let agent_id = uuid::Uuid::parse_str(&a.agent_id)
                    .map_err(|e| ControlClientError::Server(format!("invalid agent_id: {e}")))?;
                let status: AgentLifecycleStatus = serde_json::from_str(&a.status_json)?;
                Ok(AgentInfo {
                    agent_id,
                    kind: a.kind,
                    status,
                })
            })
            .collect()
    }

    pub async fn get_status(
        &mut self,
        agent_id: uuid::Uuid,
    ) -> Result<AgentLifecycleStatus, ControlClientError> {
        let resp = self
            .inner
            .get_status(AgentIdRequest {
                agent_id: agent_id.to_string(),
            })
            .await?
            .into_inner();
        Ok(serde_json::from_str(&resp.status_json)?)
    }

    pub async fn get_pool_models(&mut self) -> Result<Vec<String>, ControlClientError> {
        let resp = self.inner.get_pool_models(Empty {}).await?.into_inner();
        Ok(resp.models)
    }

    pub async fn configure_pool(
        &mut self,
        config: &ModelConfig,
    ) -> Result<(), ControlClientError> {
        let config_json = serde_json::to_string(config)?;
        let ack = self
            .inner
            .configure_pool(ConfigurePoolRequest { config_json })
            .await?
            .into_inner();
        check_ack(ack)
    }

    pub async fn list_kinds(&mut self) -> Result<Vec<String>, ControlClientError> {
        let resp = self.inner.list_kinds(Empty {}).await?.into_inner();
        Ok(resp.kinds)
    }

    /// Subscribe to the aggregated event stream. Each item is a parsed
    /// [`ProcessEvent`]; the stream ends when the daemon closes it.
    pub async fn stream_events(
        &mut self,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<ProcessEvent, ControlClientError>>,
        ControlClientError,
    > {
        use tokio_stream::StreamExt as _;
        let streaming = self.inner.stream_events(Empty {}).await?.into_inner();
        Ok(streaming.map(|item| {
            let env = item?;
            let event: ProcessEvent = serde_json::from_str(&env.payload_json)?;
            Ok(event)
        }))
    }

    /// Gracefully shut the daemon down.
    pub async fn shutdown(&mut self) -> Result<(), ControlClientError> {
        let ack = self.inner.shutdown(Empty {}).await?.into_inner();
        check_ack(ack)
    }

    /// Liveness probe (used by `daemon status`).
    pub async fn ping(&mut self) -> Result<(), ControlClientError> {
        let ack = self.inner.ping(Empty {}).await?.into_inner();
        check_ack(ack)
    }

    // ── Skill tree management ──

    /// List skills, optionally filtered.
    pub async fn list_skills(
        &mut self,
        user_invocable_only: bool,
        model_invocable_only: bool,
    ) -> Result<Vec<SkillWire>, ControlClientError> {
        use agentik_control_proto::control_plane::ListSkillsRequest;
        let resp = self
            .inner
            .list_skills(ListSkillsRequest {
                user_invocable_only,
                model_invocable_only,
            })
            .await?
            .into_inner();
        Ok(serde_json::from_str(&resp.skills_json)?)
    }

    /// Get a single skill by name/alias.
    pub async fn get_skill(&mut self, name: &str) -> Result<Option<SkillWire>, ControlClientError> {
        use agentik_control_proto::control_plane::{
            get_skill_response::Result as GetSkillResult, GetSkillRequest,
        };
        let resp = self
            .inner
            .get_skill(GetSkillRequest {
                name: name.to_string(),
            })
            .await?
            .into_inner();
        match resp.result {
            Some(GetSkillResult::SkillJson(json)) => Ok(Some(serde_json::from_str(&json)?)),
            Some(GetSkillResult::Error(_)) => Ok(None),
            None => Ok(None),
        }
    }

    /// Get the full skill tree (root node).
    pub async fn get_skill_tree(
        &mut self,
    ) -> Result<Option<SkillTreeNodeWire>, ControlClientError> {
        use agentik_control_proto::control_plane::{
            get_skill_tree_response::Result as TreeResult,
        };
        let resp = self.inner.get_skill_tree(Empty {}).await?.into_inner();
        match resp.result {
            Some(TreeResult::TreeJson(json)) => Ok(Some(serde_json::from_str(&json)?)),
            Some(TreeResult::Error(_)) => Ok(None),
            None => Ok(None),
        }
    }

    /// Reload a skill from its source. Returns the reloaded skill, or `None`
    /// if it was unchanged.
    pub async fn reload_skill(
        &mut self,
        name: &str,
    ) -> Result<Option<SkillWire>, ControlClientError> {
        use agentik_control_proto::control_plane::{
            reload_skill_response::Result as ReloadResult, ReloadSkillRequest,
        };
        let resp = self
            .inner
            .reload_skill(ReloadSkillRequest {
                name: name.to_string(),
            })
            .await?
            .into_inner();
        match resp.result {
            Some(ReloadResult::SkillJson(json)) => Ok(Some(serde_json::from_str(&json)?)),
            Some(ReloadResult::NotChanged(_)) => Ok(None),
            Some(ReloadResult::Error(_)) => Ok(None),
            None => Ok(None),
        }
    }

    /// Import all skills from a directory into the store (full replace).
    /// Returns the number imported. The daemon refreshes agent kinds after.
    pub async fn import_skills(&mut self, dir: &str) -> Result<u32, ControlClientError> {
        use agentik_control_proto::control_plane::ImportSkillsRequest;
        let resp = self
            .inner
            .import_skills(ImportSkillsRequest {
                dir: dir.to_string(),
            })
            .await?
            .into_inner();
        result_or_error(resp.count, resp.error)
    }

    /// Export all skills from the store to a directory. Returns the count.
    pub async fn export_skills(&mut self, dir: &str) -> Result<u32, ControlClientError> {
        use agentik_control_proto::control_plane::ExportSkillsRequest;
        let resp = self
            .inner
            .export_skills(ExportSkillsRequest {
                dir: dir.to_string(),
            })
            .await?
            .into_inner();
        result_or_error(resp.count, resp.error)
    }

    /// Subscribe to skill change notifications.
    pub async fn watch_skills(
        &mut self,
    ) -> Result<
        impl tokio_stream::Stream<Item = Result<SkillChangeNotificationWire, ControlClientError>>,
        ControlClientError,
    > {
        use tokio_stream::StreamExt as _;
        let streaming = self.inner.watch_skills(Empty {}).await?.into_inner();
        Ok(streaming.map(|item| {
            let env = item?;
            let notif: SkillChangeNotificationWire = serde_json::from_str(&env.payload_json)?;
            Ok(notif)
        }))
    }
}

/// Return the count on success, or map a non-empty error string to an error.
fn result_or_error(count: u32, error: String) -> Result<u32, ControlClientError> {
    if error.is_empty() {
        Ok(count)
    } else {
        Err(ControlClientError::Server(error))
    }
}

fn check_ack(ack: agentik_control_proto::control_plane::Ack) -> Result<(), ControlClientError> {
    if ack.ok {
        Ok(())
    } else {
        Err(ControlClientError::Server(ack.error))
    }
}
