pub mod utils;

use crate::utils::setup_logging;

use scouter_contracts::ServiceInfo;

use scouter_events::producer::redis::{producer::redis_producer::RedisProducer, RedisConfig};
use scouter_sql::sql::traits::SpcSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{ServerRecord, ServerRecords, SpcServerRecord};
use std::time::{Duration, Instant};
use utils::TestHelper;

trait RedisMQSetup {
    async fn start_background_producer(&self);
}

impl RedisMQSetup for TestHelper {
    async fn start_background_producer(&self) {
        let config = RedisConfig {
            address: self.config.redis_settings.as_ref().unwrap().address.clone(),
            channel: self.config.redis_settings.as_ref().unwrap().channel.clone(),
        };

        let mut producer = RedisProducer::new(config).await.unwrap();

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
                producer.publish(server_records).await.unwrap();
            }
        });
    }
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
