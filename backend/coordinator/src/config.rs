//! Structured configuration for the Coordinator service.
//!
//! All values are parsed from environment variables at startup.

use std::collections::HashMap;

/// Coordinator configuration parsed from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// HTTP server port.
    pub port: u16,
    /// Solana RPC endpoint (Devnet).
    pub solana_rpc_url: String,
    /// Map of node ID to base URL.
    pub node_urls: HashMap<String, String>,
}

impl Config {
    /// Parse configuration from environment variables.
    ///
    /// Uses sensible defaults for local development when variables are unset.
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://frost:frost@localhost:5432/coordinator_db".into());

        let port: u16 = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);

        let solana_rpc_url = std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".into());

        let node_a_url = std::env::var("NODE_A_URL")
            .unwrap_or_else(|_| "http://node-a:8081".into());
        let node_b_url = std::env::var("NODE_B_URL")
            .unwrap_or_else(|_| "http://node-b:8082".into());

        let mut node_urls = HashMap::new();
        node_urls.insert("node-a".to_string(), node_a_url);
        node_urls.insert("node-b".to_string(), node_b_url);

        Self {
            database_url,
            port,
            solana_rpc_url,
            node_urls,
        }
    }

    /// Get the base URL for a node by ID.
    pub fn node_url(&self, node_id: &str) -> Option<&str> {
        self.node_urls.get(node_id).map(|s| s.as_str())
    }
}
