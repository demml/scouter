pub mod utils;

use crate::utils::setup_logging;

use scouter_sql::sql::traits::SpcSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{ServerRecord, ServerRecords, ServiceInfo, SpcServerRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;

#[cfg(feature = "rabbitmq")]
use lapin::{
    options::{BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};

trait RabbitMQSetup {
    async fn start_background_producer(&self);
}

impl RabbitMQSetup for TestHelper {
    async fn start_background_producer(&self) {
        let config = self.config.rabbitmq_settings.clone().unwrap();

        let conn = Connection::connect(&config.address, ConnectionProperties::default())
            .await
            .unwrap();
        let channel = conn.create_channel().await.unwrap();
        channel
            .queue_declare(
                &config.queue,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();

        tokio::spawn(async move {
            loop {
                let mut records = Vec::new();
                for i in 0..15 {
                    let record = ServerRecord::Spc(SpcServerRecord {
                        created_at: chrono::Utc::now(),
                        name: "test".to_string(),
                        space: "test".to_string(),
                        feature: "feature".to_string(),
                        value: i as f64,
                        version: "1.0.0".to_string(),
                    });
                    records.push(record);
                }

                let server_records = ServerRecords { records };

                let _confirm = channel
                    .basic_publish(
                        "",
                        &config.queue,
                        BasicPublishOptions::default(),
                        &serde_json::to_string(&server_records).unwrap().into_bytes(),
                        BasicProperties::default(),
                    )
                    .await
                    .unwrap();
            }
        });
    }

    //async fn start_background_consumer(&self) {
    //    //let settings = self.config.kafka_settings.as_ref().unwrap();
    //    //
    //    //let consumer = create_kafka_consumer(&settings, None).await.unwrap();
    //    //let msg_handler = MessageHandler::Postgres(self.db_client.clone());
    //    //
    //    //(consumer, msg_handler)
    //    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());
    //    RabbitMQConsumerManager::start_workers(
    //        &self.config.rabbitmq_settings.as_ref().unwrap().clone(),
    //        &self.config.database_settings.clone(),
    //        &self.db_client.pool,
    //        shutdown_rx,
    //    )
    //    .await
    //    .unwrap();
    //}
}

#[tokio::main]
async fn main() {
    let timestamp = chrono::Utc::now();
    setup_logging().await.unwrap();

    let helper = TestHelper::new().await;

    helper.start_background_producer().await;
    //helper.start_background_consumer().await;

    let start = Instant::now();

    loop {
        if start.elapsed() < Duration::from_secs(15) {
            // Warming up
        } else {
            let service_info = ServiceInfo {
                space: "test".to_string(),
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            };
            let records = PostgresClient::get_spc_drift_records(
                &helper.db_pool,
                &service_info,
                &timestamp,
                &["feature".to_string()],
            )
            .await
            .unwrap();
            assert!(records.features["feature"].values.len() > 5000);
            break;
        }
    }
}
