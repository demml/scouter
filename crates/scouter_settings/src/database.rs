use chrono::Duration;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseSettings {
    pub connection_uri: String,
    pub max_connections: u32,
    pub retention_period: i32,
    pub flush_interval: Duration,
    pub stale_threshold: Duration,
    pub max_cache_size: usize,
    pub entity_cache_size: u64,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        let connection_uri = std::env::var("DATABASE_URI")
            .unwrap_or("postgresql://postgres:postgres@localhost:5432/postgres".to_string());

        let max_connections = std::env::var("MAX_POOL_SIZE")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<u32>()
            .unwrap();

        let retention_period = std::env::var("DATA_RETENTION_PERIOD")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<i32>()
            .unwrap();

        let flush_interval = std::env::var("TRACE_FLUSH_INTERVAL_SECONDS")
            .unwrap_or_else(|_| "15".to_string())
            .parse::<i64>()
            .map(Duration::seconds)
            .unwrap();

        let stale_threshold = std::env::var("TRACE_STALE_THRESHOLD_SECONDS")
            .unwrap_or_else(|_| "30".to_string())
            .parse::<i64>()
            .map(Duration::seconds)
            .unwrap();

        let max_cache_size = std::env::var("TRACE_CACHE_MAX_SIZE")
            .unwrap_or_else(|_| "10000".to_string())
            .parse::<usize>()
            .unwrap();

        let entity_cache_size = std::env::var("ENTITY_CACHE_MAX_SIZE")
            .unwrap_or_else(|_| "1000".to_string())
            .parse::<u64>()
            .unwrap();

        Self {
            connection_uri,
            max_connections,
            retention_period,
            flush_interval,
            stale_threshold,
            max_cache_size,
            entity_cache_size,
        }
    }
}
