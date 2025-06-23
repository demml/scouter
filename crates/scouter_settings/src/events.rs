use scouter_types::ProfileFuncs;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct KafkaSettings {
    pub brokers: String,
    pub num_workers: usize,
    pub topics: Vec<String>,
    pub group_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub security_protocol: String,
    pub sasl_mechanism: String,
    pub offset_reset: String,
    pub cert_location: Option<String>,
}

impl KafkaSettings {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl std::fmt::Debug for KafkaSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KafkaSettings")
            .field("brokers", &self.brokers)
            .field("num_workers", &self.num_workers)
            .field("topics", &self.topics)
            .field("group_id", &self.group_id)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("security_protocol", &self.security_protocol)
            .field("offset_reset", &self.offset_reset)
            .field("sasl_mechanism", &self.sasl_mechanism)
            .finish()
    }
}

impl Default for KafkaSettings {
    fn default() -> Self {
        let brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());

        let num_workers = std::env::var("KAFKA_WORKER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap();

        let topics = std::env::var("KAFKA_TOPIC")
            .unwrap_or_else(|_| "scouter_monitoring".to_string())
            .split(',')
            .map(|s| s.to_string())
            .collect();

        let group_id = std::env::var("KAFKA_GROUP").unwrap_or("scouter".to_string());
        let offset_reset = std::env::var("KAFKA_OFFSET_RESET")
            .unwrap_or_else(|_| "earliest".to_string())
            .to_string();
        let username: Option<String> = std::env::var("KAFKA_USERNAME").ok();
        let password: Option<String> = std::env::var("KAFKA_PASSWORD").ok();

        let security_protocol = std::env::var("KAFKA_SECURITY_PROTOCOL")
            .ok()
            .unwrap_or_else(|| "SASL_SSL".to_string());
        let sasl_mechanism = std::env::var("KAFKA_SASL_MECHANISM")
            .ok()
            .unwrap_or_else(|| "PLAIN".to_string());
        let cert_location = std::env::var("KAFKA_CERT_LOCATION").ok();

        Self {
            brokers,
            num_workers,
            topics,
            group_id,
            username,
            password,
            security_protocol,
            sasl_mechanism,
            offset_reset,
            cert_location,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RabbitMQSettings {
    pub num_consumers: usize,
    pub prefetch_count: u16,
    pub queue: String,
    pub consumer_tag: String,
    pub address: String,
}

impl RabbitMQSettings {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl Default for RabbitMQSettings {
    fn default() -> Self {
        let num_consumers = std::env::var("RABBITMQ_CONSUMER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap();

        let prefetch_count = std::env::var("RABBITMQ_PREFETCH_COUNT")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u16>()
            .unwrap();

        let address = std::env::var("RABBITMQ_ADDR")
            .unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5672/%2f".to_string());

        let queue =
            std::env::var("RABBITMQ_QUEUE").unwrap_or_else(|_| "scouter_monitoring".to_string());

        let consumer_tag =
            std::env::var("RABBITMQ_CONSUMER_TAG").unwrap_or_else(|_| "scouter".to_string());

        Self {
            num_consumers,
            prefetch_count,
            queue,
            consumer_tag,
            address,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RedisSettings {
    pub num_consumers: usize,
    pub channel: String,
    pub address: String,
}

impl RedisSettings {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl Default for RedisSettings {
    fn default() -> Self {
        let num_consumers = std::env::var("REDIS_CONSUMER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap();
        let channel =
            std::env::var("REDIS_CHANNEL").unwrap_or_else(|_| "scouter_monitoring".to_string());

        let address =
            std::env::var("REDIS_ADDR").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        Self {
            num_consumers,
            channel,
            address,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpConsumerSettings {
    pub num_workers: usize,
}
impl Default for HttpConsumerSettings {
    fn default() -> Self {
        let num_workers = std::env::var("HTTP_CONSUMER_WORKER_COUNT")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<usize>()
            .unwrap();

        Self { num_workers }
    }
}
