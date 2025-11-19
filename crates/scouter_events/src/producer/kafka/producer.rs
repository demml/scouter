#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub mod kafka_producer {
    use crate::producer::kafka::types::KafkaConfig;

    use crate::error::EventError;
    use futures::TryFutureExt;
    use rdkafka::{
        config::RDKafkaLogLevel,
        producer::{FutureProducer, FutureRecord, Producer},
        ClientConfig,
    };
    use rusty_logging::logger::LogLevel;
    use scouter_types::MessageRecord;
    use std::result::Result::Ok;

    use std::collections::HashMap;
    use std::time::Duration;
    use tracing::{debug, info};

    #[derive(Clone)]
    pub struct KafkaProducer {
        pub config: KafkaConfig,
        producer: FutureProducer,
    }

    impl KafkaProducer {
        pub fn new(config: KafkaConfig) -> Result<Self, EventError> {
            let producer = KafkaProducer::setup_producer(&config.config, &config.log_level)?;

            Ok(KafkaProducer { config, producer })
        }

        pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
            let mut retries = self.config.max_retries;

            loop {
                match self._publish(&message).await {
                    Ok(_) => {
                        break;
                    }
                    Err(e) => {
                        retries -= 1;
                        if retries == 0 {
                            return Err(e);
                        };
                    }
                }
            }
            debug!("Published {} records to Kafka", message.len());
            Ok(())
        }

        pub fn flush(&self) -> Result<(), EventError> {
            self.producer
                .flush(Duration::from_millis(self.config.message_timeout_ms))
                .map_err(EventError::FlushKafkaProducerError)
        }

        fn setup_producer(
            config: &HashMap<String, String>,
            log_level: &LogLevel,
        ) -> Result<rdkafka::producer::FutureProducer, EventError> {
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
                .map_err(EventError::CreateKafkaProducerError)?;

            info!("Kafka producer setup complete");
            Ok(producer)
        }

        pub async fn _publish(&self, message: &MessageRecord) -> Result<(), EventError> {
            let serialized_msg = serde_json::to_string(message).unwrap();

            let record: FutureRecord<'_, (), String> =
                FutureRecord::to(&self.config.topic).payload(&serialized_msg);

            self.producer
                .send(
                    record,
                    Duration::from_millis(self.config.message_timeout_ms),
                )
                .map_err(|(e, _)| EventError::PublishKafkaMessageError(e))
                .await?;
            Ok(())
        }
    }
}
