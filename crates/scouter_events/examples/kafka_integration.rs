pub mod utils;

use crate::utils::setup_logging;

use scouter_sql::sql::traits::SpcSqlLogic;
use scouter_sql::{sql::traits::EntitySqlLogic, PostgresClient};
use scouter_types::{ServerRecord, ServerRecords, SpcRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;

#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
use rdkafka::{
    config::RDKafkaLogLevel,
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};

trait KafkaSetup {
    async fn start_background_producer(&self, entity_uid: &str);

    //async fn start_background_consumer(&self) -> tokio::sync::watch::Sender<()>;
}

impl KafkaSetup for TestHelper {
    async fn start_background_producer(&self, entity_uid: &str) {
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
        let entity_uid = entity_uid.to_string();
        tokio::spawn(async move {
            loop {
                let mut records = Vec::new();
                for i in 0..15 {
                    let record = ServerRecord::Spc(SpcRecord {
                        created_at: chrono::Utc::now(),
                        feature: "feature".to_string(),
                        value: i as f64,
                        uid: entity_uid.clone(),
                        entity_id: 0 as i32,
                    });
                    records.push(record);
                }

                let server_records = ServerRecords { records };

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

    //async fn start_background_consumer(&self) -> tokio::sync::watch::Sender<()> {
    //    //let settings = self.config.kafka_settings.as_ref().unwrap();
    //    //
    //    //let consumer = create_kafka_consumer(&settings, None).await.unwrap();
    //    //let msg_handler = MessageHandler::Postgres(self.db_client.clone());
    //    //
    //    //(consumer, msg_handler)
    //    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    //    let kafka_settings = self.config.kafka_settings.as_ref().unwrap().clone();
    //    let db_settings = self.config.database_settings.clone();
    //    let db_pool = self.db_client.pool.clone();
    //
    //    KafkaConsumerManager::start_workers(&kafka_settings, &db_settings, &db_pool, shutdown_rx)
    //        .await
    //        .unwrap();
    //
    //    // Store or use the manager and sender as needed
    //
    //    shutdown_tx
    //}
}

#[tokio::main]
async fn main() {
    let timestamp = chrono::Utc::now();
    setup_logging().await.unwrap();

    let helper = TestHelper::new().await;

    // create entity in the database before starting consumer
    let (entity_uid, entity_id) =
        PostgresClient::create_entity(&helper.db_pool, "kafka-test", "kafka-test", "1.0.0", "spc")
            .await
            .unwrap();

    helper.start_background_producer(&entity_uid).await;

    let start = Instant::now();

    loop {
        if start.elapsed() < Duration::from_secs(15) {
            // Warming up
        } else {
            let records = PostgresClient::get_spc_drift_records(
                &helper.db_pool,
                &timestamp,
                &["feature".to_string()],
                &entity_id,
            )
            .await
            .unwrap();

            assert!(records.features["feature"].values.len() > 5000);
            break;
        }
    }
}
