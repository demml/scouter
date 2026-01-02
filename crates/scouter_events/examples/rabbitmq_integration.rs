pub mod utils;

use crate::utils::setup_logging;

#[cfg(feature = "rabbitmq")]
use lapin::{
    options::{BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties,
};
use scouter_sql::sql::traits::EntitySqlLogic;
use scouter_sql::sql::traits::SpcSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{ServerRecord, ServerRecords, SpcRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;

trait RabbitMQSetup {
    async fn start_background_producer(&self, entity_uid: &str);
}

impl RabbitMQSetup for TestHelper {
    async fn start_background_producer(&self, entity_uid: &str) {
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
        let entity_uid = entity_uid.to_string();
        tokio::spawn(async move {
            loop {
                let mut records = Vec::new();
                for i in 0..15 {
                    let record = ServerRecord::Spc(SpcRecord {
                        created_at: chrono::Utc::now(),
                        uid: entity_uid.clone(),
                        feature: "feature".to_string(),
                        value: i as f64,
                        entity_id: None,
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
}

#[tokio::main]
async fn main() {
    let timestamp = chrono::Utc::now();
    setup_logging().await.unwrap();

    let helper = TestHelper::new().await;

    let (entity_uid, entity_id) = PostgresClient::create_entity(
        &helper.db_pool,
        "rabbitmq-test",
        "rabbitmq-test",
        "1.0.0",
        "spc",
    )
    .await
    .unwrap();

    helper.start_background_producer(&entity_uid).await;
    //helper.start_background_consumer().await;

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
