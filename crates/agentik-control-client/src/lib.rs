//! gRPC client for the agentik control plane.
//!
//! Connects to a running `agentik-runtime` daemon and exposes the agent
//! lifecycle / observation API as typed Rust methods. All complex payloads are
//! serde-serialised to JSON across the wire (see `agentik-control-proto`).
//!
//! Frontends should depend on this crate (plus [`agentik_api`]) instead of
//! `agentik-runtime`, so no backend code is linked into the frontend binary.

pub mod client;
pub mod discovery;

pub use client::{ControlClient, ControlClientError};
pub use discovery::connect_to_daemon;

// Re-export the wire types so frontends have a single dependency surface.
pub use agentik_api::{
    AgentEvent, AgentLifecycleStatus, AgentSpawnOpts, AgentUiEvent, ContentBlock, ContentBlockKind,
    DaemonInfo, Message, ModelConfig, PoolEntry, ProcessEvent, ProcessExitStatus, ProviderConfig,
};
