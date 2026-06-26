//! Built-in lifecycle tools: abort_task.
//!
//! These are the always-injected tools that the agent framework requires
//! for task signaling. Primitive tools (bash, read, write, etc.) live in
//! the `agentik-tools` crate.

pub mod lifecycle;

pub use lifecycle::{
    AbortTaskInput, AbortTaskTool,
    lifecycle_registrations,
};
