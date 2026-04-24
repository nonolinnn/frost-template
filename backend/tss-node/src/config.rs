//! Structured configuration for the TSS Node service.
//!
//! All values are parsed from environment variables at startup.

/// TSS Node configuration parsed from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// HTTP server port.
    pub port: u16,
    /// This node's identifier (`node-a` or `node-b`).
    pub node_id: String,
    /// Coordinator base URL (for potential callback use).
    pub coordinator_url: String,
}

impl Config {
    /// Parse configuration from environment variables.
    ///
    /// Uses sensible defaults for local development when variables are unset.
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://frost:frost@localhost:5432/node_a_db".into());

        let port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8081);

        let node_id =
            std::env::var("NODE_ID").unwrap_or_else(|_| "node-a".into());

        let coordinator_url = std::env::var("COORDINATOR_URL")
            .unwrap_or_else(|_| "http://localhost:8080".into());

        Self {
            database_url,
            port,
            node_id,
            coordinator_url,
        }
    }
}
