//! Structured configuration for the Coordinator service.
//!
//! All values are parsed from environment variables at startup.

/// Coordinator configuration parsed from environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    /// PostgreSQL connection string.
    pub database_url: String,
    /// HTTP server port.
    pub port: u16,
    /// Solana RPC endpoint (Devnet).
    pub solana_rpc_url: String,
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

        Self {
            database_url,
            port,
            solana_rpc_url,
        }
    }
}
