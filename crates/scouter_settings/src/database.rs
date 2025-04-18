use scouter_error::ConfigError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseSettings {
    pub connection_uri: String,
    pub max_connections: u32,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        let connection_uri = std::env::var("DATABASE_URI")
            .unwrap_or("postgresql://postgres:postgres@localhost:5432/postgres".to_string());

        let max_connections = std::env::var("MAX_SQL_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u32>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        Self {
            connection_uri,
            max_connections,
        }
    }
}
