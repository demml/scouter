#[cfg(feature = "kafka")]
pub mod kafka_producer {
    use crate::producer::kafka::types::KafkaConfig;
    use rusty_logging::logger::LogLevel;
    use rdkafka::{
        config::RDKafkaLogLevel,
        producer::{FutureProducer, FutureRecord},
        ClientConfig,
    };
    use scouter_error::ScouterError;
    use std::result::Result::Ok;

    use pyo3::prelude::*;
    use std::collections::HashMap;
    use tracing::info;

    // Get table name constant
    #[pyclass]
    pub struct KafkaProducer{
        #[pyo3(get)]
        pub config:KafkaConfig,

        max_retries: i32,
        producer: FutureProducer
    }

    #[pymethods]
    impl KafkaProducer {
        #[new]
        #[pyo3(signature = (config, max_retries=3))]
        pub fn new(
            config: KafkaConfig,
            max_retries: Option<i32>,
        ) -> PyResult<Self> {

            let max_retries = max_retries.unwrap_or(3);
            let producer = KafkaProducer::setup_producer(&config.config, &config.log_level)?;

            Ok(KafkaProducer { config, max_retries, producer })
        }
    }

    impl KafkaProducer {
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
    }

}
