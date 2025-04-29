use scouter_error::ConfigError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseSettings {
    pub connection_uri: String,
    pub max_connections: u32,
    pub retention_period: i32,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        let connection_uri = std::env::var("DATABASE_URI")
            .unwrap_or("postgresql://postgres:postgres@localhost:5432/postgres".to_string());

        let max_connections = std::env::var("MAX_POOL_SIZE")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u32>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        let retention_period = std::env::var("DATA_RETENTION_PERIOD")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<i32>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        Self {
            connection_uri,
            max_connections,
            retention_period,
        }
    }
}
