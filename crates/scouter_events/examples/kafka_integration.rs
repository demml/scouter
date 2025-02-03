pub mod utils;

use crate::utils::setup_logging;

use scouter_contracts::ServiceInfo;
use scouter_types::{RecordType, ServerRecord, ServerRecords, SpcServerRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(feature = "kafka")]
use rdkafka::{
    config::RDKafkaLogLevel,
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};

#[cfg(feature = "kafka")]
use scouter_events::consumer::kafka::KafkaConsumerManager;

trait KafkaSetup {
    async fn start_background_producer(&self);

    async fn start_background_consumer(&self);
}

impl KafkaSetup for TestHelper {
    async fn start_background_producer(&self) {
        let kafka_brokers =
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_owned());

        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &kafka_brokers)
            .set("statistics.interval.ms", "500")
            .set("message.timeout.ms", "5000")
            .set_log_level(RDKafkaLogLevel::Error)
            .create()
            .expect("Producer creation error");

        let topic_name = self.config.kafka_settings.as_ref().unwrap().topics[0].clone();
        tokio::spawn(async move {
            loop {
                let mut records = Vec::new();
                for i in 0..15 {
                    let record = ServerRecord::Spc(SpcServerRecord {
                        created_at: chrono::Utc::now().naive_utc(),
                        name: "test".to_string(),
                        repository: "test".to_string(),
                        feature: "feature".to_string(),
                        value: i as f64,
                        version: "1.0.0".to_string(),
                        record_type: RecordType::Spc,
                    });
                    records.push(record);
                }

                let server_records = ServerRecords {
                    record_type: RecordType::Spc,
                    records,
                };

                let record_string = serde_json::to_string(&server_records).unwrap();

                producer
                    .send_result(
                        FutureRecord::to(&topic_name)
                            .key("key")
                            .payload(&record_string),
                    )
                    .unwrap()
                    .await
                    .unwrap()
                    .unwrap();
            }
        });
    }

    async fn start_background_consumer(&self) {
        //let settings = self.config.kafka_settings.as_ref().unwrap();
        //
        //let consumer = create_kafka_consumer(&settings, None).await.unwrap();
        //let msg_handler = MessageHandler::Postgres(self.db_client.clone());
        //
        //(consumer, msg_handler)
        #[cfg(feature = "kafka")]
        let kafka_settings = self.config.kafka_settings.as_ref().unwrap().clone();
        let db_settings = self.config.database_settings.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        KafkaConsumerManager::start_workers(&kafka_settings, &db_settings, &self.db_client.pool, shutdown)
            .await
            .unwrap();
    }
}

#[tokio::main]
async fn main() {
    let timestamp = chrono::Utc::now().naive_utc();
    setup_logging().await.unwrap();

    let helper = TestHelper::new().await;

    helper.start_background_producer().await;
    helper.start_background_consumer().await;

    let start = Instant::now();

    loop {
        if start.elapsed() < Duration::from_secs(15) {
            // Warming up
        } else {
            let service_info = ServiceInfo {
                repository: "test".to_string(),
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            };
            let records = helper
                .db_client
                .get_spc_drift_records(&service_info, &timestamp, &["feature".to_string()])
                .await
                .unwrap();

            assert!(records.features["feature"].values.len() > 5000);
            break;
        }
    }

    //startup_kafka(&helper.db_client.pool, &helper.config).await.unwrap();

    //helper.populate_kafka().await.unwrap();
}
