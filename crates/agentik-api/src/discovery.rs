//! Daemon discovery — the file the runtime daemon writes so clients (CLI /
//! TUI) can find its control-plane address without a fixed port.
//!
//! Location: `<state_dir>/agentik/daemon.json`, where `state_dir` is
//! `$XDG_STATE_HOME` or `~/.local/state`.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Daemon discovery record written at startup and removed on shutdown.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonInfo {
    /// Control-plane gRPC address (e.g. `127.0.0.1:54321`).
    pub control_addr: SocketAddr,
    /// Embedded skill-registry gRPC address, if running.
    pub skill_addr: Option<SocketAddr>,
    /// OS process id of the daemon.
    pub pid: u32,
    /// Unix epoch seconds at which the daemon started.
    pub started_at: u64,
}

/// Resolve the per-user state directory (`$XDG_STATE_HOME` or `~/.local/state`).
pub fn state_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local").join("state"))
}

/// `<state_dir>/agentik`.
pub fn agentik_dir() -> Option<PathBuf> {
    Some(state_dir()?.join("agentik"))
}

/// Path to the `daemon.json` discovery file.
pub fn daemon_json_path() -> Option<PathBuf> {
    Some(agentik_dir()?.join("daemon.json"))
}

/// Read and parse the discovery file. Returns `None` if missing or unreadable.
pub fn read_daemon_info() -> Option<DaemonInfo> {
    let path = daemon_json_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}
