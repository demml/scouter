#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub mod kafka_producer {
    use crate::producer::kafka::types::KafkaConfig;

    use futures::TryFutureExt;
    use rdkafka::{
        config::RDKafkaLogLevel,
        producer::{FutureProducer, FutureRecord, Producer},
        ClientConfig,
    };
    use rusty_logging::logger::LogLevel;
    use scouter_error::ScouterError;
    use scouter_types::ServerRecords;
    use std::result::Result::Ok;

    use std::collections::HashMap;
    use std::time::Duration;
    use tracing::{error, info};

    #[derive(Clone)]
    pub struct KafkaProducer {
        pub config: KafkaConfig,
        producer: FutureProducer,
    }

    impl KafkaProducer {
        pub fn new(config: KafkaConfig) -> Result<Self, ScouterError> {
            let producer = KafkaProducer::setup_producer(&config.config, &config.log_level)?;

            Ok(KafkaProducer { config, producer })
        }

        pub async fn publish(&self, message: ServerRecords) -> Result<(), ScouterError> {
            let mut retries = self.config.max_retries;

            loop {
                match self
                    ._publish(message.clone())
                    .await
                    .map_err(|e| ScouterError::Error(e.to_string()))
                {
                    Ok(_) => {
                        break;
                    }
                    Err(e) => {
                        retries -= 1;
                        if retries == 0 {
                            return {
                                error!("Failed to send message to kafka: {:?}", e.to_string());
                                Err(ScouterError::Error(format!(
                                    "Failed to send message to kafka: {:?}",
                                    e.to_string()
                                )))
                            };
                        }
                    }
                }
            }

            Ok(())
        }

        pub fn flush(&self) -> Result<(), ScouterError> {
            self.producer
                .flush(Duration::from_millis(self.config.message_timeout_ms))
                .map_err(|e| ScouterError::Error(e.to_string()))
        }

        fn setup_producer(
            config: &HashMap<String, String>,
            log_level: &LogLevel,
        ) -> Result<rdkafka::producer::FutureProducer, ScouterError> {
            info!("Setting up Kafka producer");
            let mut kafka_config = ClientConfig::new();

            config.iter().for_each(|(key, value)| {
                kafka_config.set(key, value);
            });

            let kafka_log_level = match log_level {
                LogLevel::Error => RDKafkaLogLevel::Error,
                LogLevel::Warn => RDKafkaLogLevel::Warning,
                LogLevel::Info => RDKafkaLogLevel::Info,
                LogLevel::Debug => RDKafkaLogLevel::Debug,
                LogLevel::Trace => RDKafkaLogLevel::Debug,
            };

            let producer = kafka_config
                .set_log_level(kafka_log_level)
                .create()
                .map_err(|e| ScouterError::Error(e.to_string()))?;

            info!("Kafka producer setup complete");
            Ok(producer)
        }

        pub async fn _publish(&self, message: ServerRecords) -> Result<(), ScouterError> {
            let serialized_msg = serde_json::to_string(&message).unwrap();

            let record: FutureRecord<'_, (), String> =
                FutureRecord::to(&self.config.topic).payload(&serialized_msg);

            self.producer
                .send(
                    record,
                    Duration::from_millis(self.config.message_timeout_ms),
                )
                .map_err(|(e, _)| ScouterError::Error(e.to_string()))
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))?;
            Ok(())
        }
    }
}
