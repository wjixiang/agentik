//! Convenience for connecting to a locally-running daemon via the discovery file.

use crate::client::{ControlClient, ControlClientError};

/// Read `<state_dir>/agentik/daemon.json` and connect a [`ControlClient`] to
/// the daemon's control address.
pub async fn connect_to_daemon() -> Result<ControlClient, ControlClientError> {
    let info = agentik_api::read_daemon_info().ok_or(ControlClientError::NoDaemon)?;
    let addr = format!("http://{}", info.control_addr);
    ControlClient::connect(&addr).await
}
