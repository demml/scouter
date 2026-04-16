use scouter_types::PyHelperFuncs;
use serde::Serialize;

// see: https://github.com/confluentinc/librdkafka/blob/master/CONFIGURATION.md
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
        PyHelperFuncs::__str__(self)
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
        PyHelperFuncs::__str__(self)
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
        PyHelperFuncs::__str__(self)
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

#[derive(Debug, Clone, Serialize)]
pub struct HttpConsumerSettings {
    pub server_record_workers: usize,
    pub trace_workers: usize,
    pub tag_workers: usize,
}

impl HttpConsumerSettings {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Default for HttpConsumerSettings {
    fn default() -> Self {
        let parse_worker_count = |var: &str, default: usize| -> usize {
            std::env::var(var)
                .ok()
                .and_then(|v| {
                    v.parse::<usize>()
                        .map_err(|_| {
                            tracing::warn!(
                                "Invalid value for {var}, using default {default}"
                            );
                        })
                        .ok()
                })
                .unwrap_or(default)
        };

        Self {
            server_record_workers: parse_worker_count("SERVER_RECORD_CONSUMER_WORKERS", 4),
            trace_workers: parse_worker_count("TRACE_CONSUMER_WORKERS", 2),
            tag_workers: parse_worker_count("TAG_CONSUMER_WORKERS", 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_consumer_settings_defaults() {
        // Unset env vars → defaults
        std::env::remove_var("SERVER_RECORD_CONSUMER_WORKERS");
        std::env::remove_var("TRACE_CONSUMER_WORKERS");
        std::env::remove_var("TAG_CONSUMER_WORKERS");
        let s = HttpConsumerSettings::default();
        assert_eq!(s.server_record_workers, 4);
        assert_eq!(s.trace_workers, 2);
        assert_eq!(s.tag_workers, 1);
    }

    #[test]
    fn test_http_consumer_settings_env_override() {
        std::env::set_var("SERVER_RECORD_CONSUMER_WORKERS", "8");
        std::env::set_var("TRACE_CONSUMER_WORKERS", "3");
        std::env::set_var("TAG_CONSUMER_WORKERS", "2");
        let s = HttpConsumerSettings::default();
        assert_eq!(s.server_record_workers, 8);
        assert_eq!(s.trace_workers, 3);
        assert_eq!(s.tag_workers, 2);
        std::env::remove_var("SERVER_RECORD_CONSUMER_WORKERS");
        std::env::remove_var("TRACE_CONSUMER_WORKERS");
        std::env::remove_var("TAG_CONSUMER_WORKERS");
    }

    #[test]
    fn test_http_consumer_settings_invalid_env_falls_back_to_default() {
        std::env::set_var("SERVER_RECORD_CONSUMER_WORKERS", "not-a-number");
        let s = HttpConsumerSettings::default();
        // Falls back to default instead of panicking
        assert_eq!(s.server_record_workers, 4);
        std::env::remove_var("SERVER_RECORD_CONSUMER_WORKERS");
    }
}
