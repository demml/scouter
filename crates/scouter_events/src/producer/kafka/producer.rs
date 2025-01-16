#[cfg(feature = "kafka")]
pub mod kafka_producer {
    use crate::producer::kafka::types::KafkaConfig;
 
    use futures::TryFutureExt;
    use rusty_logging::logger::LogLevel;
    use rdkafka::{
        config::RDKafkaLogLevel,
        producer::{FutureProducer, FutureRecord, Producer},
        ClientConfig,
    };
    use scouter_error::{PyScouterError, ScouterError};
    use scouter_types::ServerRecords;
    use std::result::Result::Ok;

    use pyo3::prelude::*;
    use std::collections::HashMap;
    use tracing::{info, debug};
    use std::time::Duration;


    pub struct KafkaProducer{
        pub config:KafkaConfig,
        pub max_retries: i32,
        producer: FutureProducer,
        rt: tokio::runtime::Runtime,
    }

    impl KafkaProducer {
        pub fn new(
            config: KafkaConfig,
            max_retries: Option<i32>,
        ) -> Result<Self, ScouterError> {

            let max_retries = max_retries.unwrap_or(3);
            let producer = KafkaProducer::setup_producer(&config.config, &config.log_level)?;
            let rt = tokio::runtime::Runtime::new().unwrap();

            Ok(KafkaProducer { config, max_retries, producer, rt })
        }

        pub fn publish(&self, message: ServerRecords) -> PyResult<()> {
            let mut retries = self.max_retries;
        
            loop {
                match self.rt.block_on(async {
                    self.future_publish(message.clone()).await.map_err(|e| PyScouterError::new_err(e.to_string()))
                }) {
                    Ok(_) => break,
                    Err(e) => {
                        retries -= 1;
                        if retries == 0 {
                            return {
                                Err(PyScouterError::new_err(format!("Failed to send message to kafka: {:?}", e.to_string())))};
                        }
                    }
                }
            }
        
            Ok(())
        }

        pub fn flush(&self) -> PyResult<()> {
            self.producer.flush(Duration::from_millis(self.config.message_timeout_ms)).map_err(|e| PyScouterError::new_err(e.to_string()))
        }

        fn setup_producer(config: &HashMap<String, String>, log_level: &LogLevel) -> Result<rdkafka::producer::FutureProducer, ScouterError> {
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

        pub async fn future_publish(&self, message: ServerRecords) -> Result<(), ScouterError> {
            let serialized_msg = serde_json::to_string(&message).unwrap();

            debug!("Publishing message to Kafka");
            let record:  FutureRecord<'_, (), String> = FutureRecord::to(&self.config.topic).payload(&serialized_msg);
       

            self.producer.send(record, Duration::from_millis(self.config.message_timeout_ms))
                .map_err(|(e, _)| ScouterError::Error(e.to_string()))
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))?;
            Ok(())
        }

  

       
    }

}
