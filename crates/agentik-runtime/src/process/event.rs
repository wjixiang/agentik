//! Process-event types.
//!
//! These now live in [`agentik_api`] (so they can cross the control-plane
//! boundary without linking `agentik-core`); re-exported here for the
//! historical `agentik_runtime::process::event` / `agentik_runtime::ProcessEvent`
//! paths.

pub use agentik_api::{ProcessEvent, ProcessExitStatus};
