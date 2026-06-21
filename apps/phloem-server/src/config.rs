//! Server configuration loaded from environment variables.

use std::fmt;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to.
    pub host: String,
    /// Port to listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("PHLOEM_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PHLOEM_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
        }
    }
}

impl fmt::Display for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}
