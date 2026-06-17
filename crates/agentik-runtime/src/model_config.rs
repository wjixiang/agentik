//! Declarative model-configuration types for the frontend.
//!
//! These are pure-data, `serde`-serialisable structs with **no dependency on
//! `agentik-core`**. They are defined in [`agentik_api`] (the shared control-
//! plane contract) and re-exported here so historical
//! `agentik_runtime::model_config` / `agentik_runtime::ModelConfig` paths keep
//! resolving.

pub use agentik_api::{ModelConfig, PoolEntry, ProviderConfig};
