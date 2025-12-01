pub mod utils;

use crate::utils::setup_logging;

use scouter_types::ServiceInfo;

use scouter_events::producer::redis::{producer::redis_producer::RedisProducer, RedisConfig};
use scouter_sql::sql::traits::EntitySqlLogic;
use scouter_sql::sql::traits::SpcSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{MessageRecord, ServerRecord, ServerRecords, SpcRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;

trait RedisMQSetup {
    async fn start_background_producer(&self, entity_uid: &str);
}

impl RedisMQSetup for TestHelper {
    async fn start_background_producer(&self, entity_uid: &str) {
        let config = RedisConfig {
            address: self.config.redis_settings.as_ref().unwrap().address.clone(),
            channel: self.config.redis_settings.as_ref().unwrap().channel.clone(),
            ..Default::default()
        };

        let producer = RedisProducer::new(config).await.unwrap();
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
                    });
                    records.push(record);
                }

                let server_records = ServerRecords { records };
                producer
                    .publish(MessageRecord::ServerRecords(server_records))
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

    let service_info = ServiceInfo {
        space: "test".to_string(),
        name: "test".to_string(),
        version: "1.0.0".to_string(),
    };
    let (entity_uid, entity_id) = PostgresClient::create_entity(
        &helper.db_pool,
        service_info.space.as_str(),
        service_info.name.as_str(),
        service_info.version.as_str(),
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
