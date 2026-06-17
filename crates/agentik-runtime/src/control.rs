//! gRPC control-plane service.
//!
//! [`AgentControlGrpcService`] wraps a [`AgentManager`] (which is `Clone`)
//! and exposes the agent lifecycle / observation API over gRPC, so that an
//! external CLI or TUI process can drive the daemon. Mirrors the structure of
//! the skill-registry service in `agentik-skill-server`.
//!
//! Complex payloads are serde-serialised to JSON across the wire (see
//! `agentik-control-proto`).

use std::pin::Pin;
use std::sync::Arc;

use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status};

use agentik_api::{
    AgentSpawnOpts, ContentBlock, ModelConfig, ProcessEvent, SkillChangeNotificationWire,
    SkillChangeWire, SkillReferenceWire, SkillTreeNodeWire, SkillWire,
};
use agentik_control_proto::control_plane::agent_control_service_server::AgentControlService;
use agentik_control_proto::control_plane::{
    get_skill_response::Result as GetSkillResult, get_skill_tree_response::Result as TreeResult,
    reload_skill_response::Result as ReloadResult, spawn_agent_response::Result as SpawnResult,
    Ack, AgentIdRequest, AgentInfo, ConfigurePoolRequest, Empty, EventEnvelope,
    ExportSkillsRequest, GetPoolModelsResponse, GetSkillRequest, GetSkillResponse,
    GetSkillTreeResponse, ImportResult, ImportSkillsRequest, InjectMessageRequest,
    ListAgentsResponse, ListKindsResponse, ListSkillsRequest, ListSkillsResponse,
    ReloadSkillRequest, ReloadSkillResponse, SkillChangeEnvelope, SpawnAgentRequest,
    SpawnAgentResponse, StatusResponse,
};
use agentik_skill::{Skill, SkillTreeNode};
use agentik_skill_client::SkillRegistryClient;
use agentik_skill_server::store::SkillStore as _;
use agentik_skill_server::SqliteSkillStore;

use tokio::sync::Mutex;

use crate::kinds;
use crate::process::AgentManager;

/// gRPC service implementing the agent control plane.
pub struct AgentControlGrpcService {
    pm: AgentManager,
    /// Shared skill store (source of truth for the skill tree). `None` when the
    /// daemon runs without a skill server.
    store: Option<Arc<SqliteSkillStore>>,
    /// Skill registry client, used when refreshing agent kinds after an import.
    skill_client: Option<Arc<Mutex<SkillRegistryClient>>>,
    /// Triggered by the `Shutdown` RPC; the daemon's main loop awaits it.
    shutdown: CancellationToken,
}

impl AgentControlGrpcService {
    pub fn new(
        pm: AgentManager,
        store: Option<Arc<SqliteSkillStore>>,
        skill_client: Option<Arc<Mutex<SkillRegistryClient>>>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            pm,
            store,
            skill_client,
            shutdown,
        }
    }

    /// Reference the store or return a `FAILED_PRECONDITION` status.
    fn store(&self) -> Result<&Arc<SqliteSkillStore>, Status> {
        self.store
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("skill store is not configured on this daemon"))
    }

    /// Refresh the built-in `coder` kind from the current store contents, so
    /// that agents spawned after a skill import see the new tree. Best-effort:
    /// failures are logged, not fatal.
    async fn refresh_coder_kind(&self, store: &Arc<SqliteSkillStore>) {
        match kinds::coder_kind(store, self.skill_client.clone()).await {
            Ok(blueprint) => {
                self.pm.registry().register(blueprint);
                tracing::info!("refreshed 'coder' kind after skill change");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to refresh 'coder' kind after skill change");
            }
        }
    }

    pub fn into_server(
        self,
    ) -> agentik_control_proto::control_plane::agent_control_service_server::AgentControlServiceServer<
        Self,
    > {
        use agentik_control_proto::control_plane::agent_control_service_server::AgentControlServiceServer;
        AgentControlServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl AgentControlService for AgentControlGrpcService {
    type StreamEventsStream =
        Pin<Box<dyn Stream<Item = Result<EventEnvelope, Status>> + Send>>;
    type WatchSkillsStream =
        Pin<Box<dyn Stream<Item = Result<SkillChangeEnvelope, Status>> + Send>>;

    async fn spawn_agent(
        &self,
        request: Request<SpawnAgentRequest>,
    ) -> Result<Response<SpawnAgentResponse>, Status> {
        let req = request.into_inner();
        let opts: AgentSpawnOpts = serde_json::from_str(&req.opts_json)
            .map_err(|e| Status::invalid_argument(format!("invalid opts_json: {e}")))?;
        match self.pm.spawn_by_kind(&req.kind, opts).await {
            Ok(id) => Ok(Response::new(SpawnAgentResponse {
                result: Some(SpawnResult::AgentId(id.to_string())),
            })),
            Err(e) => Ok(Response::new(SpawnAgentResponse {
                result: Some(SpawnResult::Error(e.to_string())),
            })),
        }
    }

    async fn start_agent(
        &self,
        request: Request<AgentIdRequest>,
    ) -> Result<Response<Ack>, Status> {
        let id = parse_id(&request.into_inner().agent_id)?;
        Ok(Response::new(to_ack(self.pm.start(&id))))
    }

    async fn stop_agent(
        &self,
        request: Request<AgentIdRequest>,
    ) -> Result<Response<Ack>, Status> {
        let id = parse_id(&request.into_inner().agent_id)?;
        Ok(Response::new(to_ack(self.pm.stop(&id))))
    }

    async fn restart_agent(
        &self,
        request: Request<AgentIdRequest>,
    ) -> Result<Response<Ack>, Status> {
        let id = parse_id(&request.into_inner().agent_id)?;
        Ok(Response::new(to_ack(self.pm.restart(&id))))
    }

    async fn inject_message(
        &self,
        request: Request<InjectMessageRequest>,
    ) -> Result<Response<Ack>, Status> {
        let req = request.into_inner();
        let id = parse_id(&req.agent_id)?;
        let content: Vec<ContentBlock> = serde_json::from_str(&req.content_json)
            .map_err(|e| Status::invalid_argument(format!("invalid content_json: {e}")))?;
        Ok(Response::new(to_ack(self.pm.inject_message(&id, content))))
    }

    async fn list_agents(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let snapshot = self.pm.snapshot().await;
        let agents = snapshot
            .into_iter()
            .map(|(id, kind, status)| AgentInfo {
                agent_id: id.to_string(),
                kind,
                status_json: serde_json::to_string(&status).unwrap_or_default(),
            })
            .collect();
        Ok(Response::new(ListAgentsResponse { agents }))
    }

    async fn get_status(
        &self,
        request: Request<AgentIdRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let id = parse_id(&request.into_inner().agent_id)?;
        match self.pm.status(&id) {
            Ok(status) => Ok(Response::new(StatusResponse {
                status_json: serde_json::to_string(&status).unwrap_or_default(),
            })),
            Err(e) => Err(Status::not_found(e.to_string())),
        }
    }

    async fn get_pool_models(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetPoolModelsResponse>, Status> {
        let models = self.pm.pool_model_names().await;
        Ok(Response::new(GetPoolModelsResponse { models }))
    }

    async fn configure_pool(
        &self,
        request: Request<ConfigurePoolRequest>,
    ) -> Result<Response<Ack>, Status> {
        let req = request.into_inner();
        let cfg: ModelConfig = serde_json::from_str(&req.config_json)
            .map_err(|e| Status::invalid_argument(format!("invalid config_json: {e}")))?;
        let result = self.pm.configure_pool(&cfg).await;
        Ok(Response::new(to_ack_async(result)))
    }

    async fn list_kinds(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListKindsResponse>, Status> {
        Ok(Response::new(ListKindsResponse {
            kinds: self.pm.registry().list(),
        }))
    }

    async fn stream_events(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::StreamEventsStream>, Status> {
        let rx = self.pm.events();
        let stream = BroadcastStream::new(rx).filter_map(|result| {
            result
                .ok()
                .map(|event: ProcessEvent| {
                    let payload_json = serde_json::to_string(&event).unwrap_or_default();
                    Ok(EventEnvelope { payload_json })
                })
        });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn shutdown(&self, _request: Request<Empty>) -> Result<Response<Ack>, Status> {
        tracing::info!("shutdown requested via control plane");
        self.shutdown.cancel();
        Ok(Response::new(Ack {
            ok: true,
            error: String::new(),
        }))
    }

    async fn ping(&self, _request: Request<Empty>) -> Result<Response<Ack>, Status> {
        Ok(Response::new(Ack {
            ok: true,
            error: String::new(),
        }))
    }

    // ── Skill tree management ──

    async fn list_skills(
        &self,
        request: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        let store = self.store()?;
        let req = request.into_inner();
        let tree = store
            .skill_tree()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut wires = Vec::new();
        flatten_tree(&tree, &mut wires);
        let filtered: Vec<SkillWire> = wires
            .into_iter()
            .filter(|w| !req.user_invocable_only || w.user_invocable)
            .filter(|w| !req.model_invocable_only || w.model_invocable)
            .collect();
        let skills_json = serde_json::to_string(&filtered)
            .map_err(|e| Status::internal(format!("encode error: {e}")))?;
        Ok(Response::new(ListSkillsResponse { skills_json }))
    }

    async fn get_skill(
        &self,
        request: Request<GetSkillRequest>,
    ) -> Result<Response<GetSkillResponse>, Status> {
        let store = self.store()?;
        let name = request.into_inner().name;
        match store.get(&name).await {
            Ok(skill) => {
                // Look up the dotpath via the tree for a complete wire view.
                let dotpath = store
                    .skill_tree()
                    .await
                    .ok()
                    .and_then(|t| find_dotpath(&t, &skill.metadata.name))
                    .unwrap_or_default();
                let wire = skill_to_wire(&dotpath, &skill);
                let skill_json = serde_json::to_string(&wire)
                    .map_err(|e| Status::internal(format!("encode error: {e}")))?;
                Ok(Response::new(GetSkillResponse {
                    result: Some(GetSkillResult::SkillJson(skill_json)),
                }))
            }
            Err(e) => Ok(Response::new(GetSkillResponse {
                result: Some(GetSkillResult::Error(e.to_string())),
            })),
        }
    }

    async fn get_skill_tree(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetSkillTreeResponse>, Status> {
        let store = self.store()?;
        match store.skill_tree().await {
            Ok(tree) => match tree.root.as_ref() {
                Some(root) => {
                    let wire = tree_to_wire(root);
                    let tree_json = serde_json::to_string(&wire)
                        .map_err(|e| Status::internal(format!("encode error: {e}")))?;
                    Ok(Response::new(GetSkillTreeResponse {
                        result: Some(TreeResult::TreeJson(tree_json)),
                    }))
                }
                None => Ok(Response::new(GetSkillTreeResponse {
                    result: Some(TreeResult::Error("skill tree is empty".to_string())),
                })),
            },
            Err(e) => Ok(Response::new(GetSkillTreeResponse {
                result: Some(TreeResult::Error(e.to_string())),
            })),
        }
    }

    async fn reload_skill(
        &self,
        request: Request<ReloadSkillRequest>,
    ) -> Result<Response<ReloadSkillResponse>, Status> {
        let store = self.store()?;
        let name = request.into_inner().name;
        match store.reload(&name).await {
            Ok(Some(skill)) => {
                let dotpath = store
                    .skill_tree()
                    .await
                    .ok()
                    .and_then(|t| find_dotpath(&t, &skill.metadata.name))
                    .unwrap_or_default();
                let wire = skill_to_wire(&dotpath, &skill);
                let skill_json = serde_json::to_string(&wire)
                    .map_err(|e| Status::internal(format!("encode error: {e}")))?;
                Ok(Response::new(ReloadSkillResponse {
                    result: Some(ReloadResult::SkillJson(skill_json)),
                }))
            }
            Ok(None) => Ok(Response::new(ReloadSkillResponse {
                result: Some(ReloadResult::NotChanged(
                    "skill is already up to date".to_string(),
                )),
            })),
            Err(e) => Ok(Response::new(ReloadSkillResponse {
                result: Some(ReloadResult::Error(e.to_string())),
            })),
        }
    }

    async fn import_skills(
        &self,
        request: Request<ImportSkillsRequest>,
    ) -> Result<Response<ImportResult>, Status> {
        let store = self.store()?.clone();
        let dir = request.into_inner().dir;
        match store.import_from_dir(std::path::Path::new(&dir)).await {
            Ok(count) => {
                self.refresh_coder_kind(&store).await;
                Ok(Response::new(ImportResult {
                    count: count as u32,
                    error: String::new(),
                }))
            }
            Err(e) => Ok(Response::new(ImportResult {
                count: 0,
                error: e.to_string(),
            })),
        }
    }

    async fn export_skills(
        &self,
        request: Request<ExportSkillsRequest>,
    ) -> Result<Response<ImportResult>, Status> {
        let store = self.store()?;
        let dir = request.into_inner().dir;
        match store.export_to_dir(std::path::Path::new(&dir)).await {
            Ok(count) => Ok(Response::new(ImportResult {
                count: count as u32,
                error: String::new(),
            })),
            Err(e) => Ok(Response::new(ImportResult {
                count: 0,
                error: e.to_string(),
            })),
        }
    }

    async fn watch_skills(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<Self::WatchSkillsStream>, Status> {
        let store = self.store()?;
        let rx = store.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(|result| {
            result.ok().map(|notif| {
                let change = match notif.change_type {
                    agentik_skill_server::SkillChangeType::Added => SkillChangeWire::Added,
                    agentik_skill_server::SkillChangeType::Modified => SkillChangeWire::Modified,
                    agentik_skill_server::SkillChangeType::Removed => SkillChangeWire::Removed,
                };
                let wire = SkillChangeNotificationWire {
                    change_type: change,
                    skill_name: notif.skill_name,
                };
                let payload_json = serde_json::to_string(&wire).unwrap_or_default();
                Ok(SkillChangeEnvelope { payload_json })
            })
        });
        Ok(Response::new(Box::pin(stream)))
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn parse_id(s: &str) -> Result<uuid::Uuid, Status> {
    uuid::Uuid::parse_str(s)
        .map_err(|e| Status::invalid_argument(format!("invalid agent_id '{s}': {e}")))
}

/// Convert a sync `Result<(), ProcessError>` into an [`Ack`].
fn to_ack(res: Result<(), crate::process::ProcessError>) -> Ack {
    match res {
        Ok(()) => Ack {
            ok: true,
            error: String::new(),
        },
        Err(e) => Ack {
            ok: false,
            error: e.to_string(),
        },
    }
}

/// Convert an already-awaited pool-configure result into an [`Ack`].
fn to_ack_async(res: Result<(), crate::process::ProcessError>) -> Ack {
    to_ack(res)
}

// ── Skill ↔ wire mapping ────────────────────────────────────────

/// Map a domain [`Skill`] to its wire form, given its dotpath.
fn skill_to_wire(dotpath: &str, skill: &Skill) -> SkillWire {
    SkillWire {
        dotpath: dotpath.to_string(),
        name: skill.metadata.name.clone(),
        description: skill.metadata.description.clone(),
        aliases: skill.metadata.aliases.clone(),
        when_to_use: skill.metadata.when_to_use.clone(),
        argument_hint: skill.metadata.argument_hint.clone(),
        user_invocable: skill.metadata.user_invocable,
        model_invocable: skill.metadata.model_invocable,
        allowed_tools: skill.policy.allowed_tools.iter().cloned().collect(),
        body: skill.body.clone(),
        references: skill
            .references
            .iter()
            .map(|r| SkillReferenceWire {
                name: r.name.clone(),
                content: r.content.clone(),
            })
            .collect(),
        activation_paths: skill.activation_paths.clone(),
    }
}

/// Map a domain [`SkillTreeNode`] tree to its wire form.
fn tree_to_wire(node: &SkillTreeNode) -> SkillTreeNodeWire {
    SkillTreeNodeWire {
        skill: skill_to_wire(&node.dotpath, &node.skill),
        dotpath: node.dotpath.clone(),
        children: node.children.iter().map(tree_to_wire).collect(),
    }
}

fn flatten_tree(tree: &agentik_skill::SkillTree, out: &mut Vec<SkillWire>) {
    if let Some(root) = &tree.root {
        flatten_node(root, out);
    }
}

fn flatten_node(node: &SkillTreeNode, out: &mut Vec<SkillWire>) {
    out.push(skill_to_wire(&node.dotpath, &node.skill));
    for child in &node.children {
        flatten_node(child, out);
    }
}

/// Find the dotpath of the first node whose skill name matches.
fn find_dotpath(tree: &agentik_skill::SkillTree, name: &str) -> Option<String> {
    tree.root.as_ref().and_then(|n| find_dotpath_node(n, name))
}

fn find_dotpath_node(node: &SkillTreeNode, name: &str) -> Option<String> {
    if node.skill.metadata.name == name {
        return Some(node.dotpath.clone());
    }
    for child in &node.children {
        if let Some(dp) = find_dotpath_node(child, name) {
            return Some(dp);
        }
    }
    None
}
