//! Multi-agent runtime layer for `agentik`.
//!
//! Hosts the [`process`] module — a [`ProcessManager`](process::ProcessManager) that
//! spawns, monitors, and controls multiple [`Agent`](agentik_core::Agent) instances as
//! independent tokio tasks.

pub mod process;

pub use process::{ProcessError, ProcessEvent, ProcessExitStatus, ProcessManager};
