use scouter_error::ConfigError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PollingSettings {
    pub num_workers: usize,
}

impl Default for PollingSettings {
    fn default() -> Self {
        let num_workers = std::env::var("SCHEDULE_WORKER_COUNT")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        Self { num_workers }
    }
}

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

#[derive(Debug, Clone, Serialize)]
pub struct KafkaSettings {
    pub brokers: String,
    pub worker_count: usize,
    pub topics: Vec<String>,
    pub group_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub security_protocol: String,
    pub sasl_mechanism: String,
}

impl Default for KafkaSettings {
    fn default() -> Self {
        let brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());

        let worker_count = std::env::var("KAFKA_WORKER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        let topics = vec![std::env::var("KAFKA_TOPIC").unwrap_or("scouter_monitoring".to_string())];

        let group_id = std::env::var("KAFKA_GROUP").unwrap_or("scouter".to_string());
        let username: Option<String> = std::env::var("KAFKA_USERNAME").ok();
        let password: Option<String> = std::env::var("KAFKA_PASSWORD").ok();

        let security_protocol = std::env::var("KAFKA_SECURITY_PROTOCOL")
            .ok()
            .unwrap_or_else(|| "SASL_SSL".to_string());
        let sasl_mechanism = std::env::var("KAFKA_SASL_MECHANISM")
            .ok()
            .unwrap_or_else(|| "PLAIN".to_string());

        Self {
            brokers,
            worker_count,
            topics,
            group_id,
            username,
            password,
            security_protocol,
            sasl_mechanism,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RabbitMQSettings {
    pub num_consumers: usize,
    pub prefetch_count: u16,
    pub address: String,
}
impl Default for RabbitMQSettings {
    fn default() -> Self {
        let num_consumers = std::env::var("RABBITMQ_CONSUMER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        let prefetch_count = std::env::var("RABBITMQ_PREFETCH_COUNT")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u16>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        let address = std::env::var("RABBITMQ_ADDRESS")
            .unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5672/%2f".to_string());

        Self {
            num_consumers,
            prefetch_count,
            address,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScouterServerConfig {
    pub server_port: u16,
    pub polling: PollingSettings,
    pub database: DatabaseSettings,
    pub kafka: Option<KafkaSettings>,
    pub rabbitmq: Option<RabbitMQSettings>,
}

impl Default for ScouterServerConfig {
    fn default() -> Self {
        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "8000".to_string())
            .parse::<u16>()
            .map_err(|e| ConfigError::Error(format!("{:?}", e)))
            .unwrap();

        let polling = PollingSettings::default();
        let database = DatabaseSettings::default();
        let kafka = if std::env::var("KAFKA_BROKERS").is_ok() {
            Some(KafkaSettings::default())
        } else {
            None
        };

        let rabbitmq = if std::env::var("RABBITMQ_ADDR").is_ok() {
            Some(RabbitMQSettings::default())
        } else {
            None
        };

        Self {
            server_port,
            polling,
            database,
            kafka,
            rabbitmq,
        }
    }
}
