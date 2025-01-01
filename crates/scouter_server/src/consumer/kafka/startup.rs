#[cfg(feature = "kafka")]
pub mod kafka_startup {

    use crate::consumer::kafka::consumer::kafka_consumer::start_kafka_background_poll;
    use anyhow::*;
    use scouter_sql::{MessageHandler, PostgresClient};
    use sqlx::{Pool, Postgres};
    use tracing::info;

    pub async fn startup_kafka(pool: &Pool<Postgres>) -> Result<()> {
        info!("Starting Kafka consumer");

        let num_kafka_workers = std::env::var("KAFKA_WORKER_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .with_context(|| "Failed to parse NUM_KAFKA_WORKERS")?;

        for _ in 0..num_kafka_workers {
            let kafka_db_client = PostgresClient::new(Some(pool.clone()))
                .await
                .with_context(|| "Failed to create Postgres client")?;
            let message_handler = MessageHandler::Postgres(kafka_db_client);
            let brokers =
                std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_owned());
            let topics =
                vec![std::env::var("KAFKA_TOPIC").unwrap_or("scouter_monitoring".to_string())];
            let group_id = std::env::var("KAFKA_GROUP").unwrap_or("scouter".to_string());
            let username: Option<String> = std::env::var("KAFKA_USERNAME").ok();
            let password: Option<String> = std::env::var("KAFKA_PASSWORD").ok();
            let security_protocol: Option<String> = Some(
                std::env::var("KAFKA_SECURITY_PROTOCOL")
                    .ok()
                    .unwrap_or_else(|| "SASL_SSL".to_string()),
            );
            let sasl_mechanism: Option<String> = Some(
                std::env::var("KAFKA_SASL_MECHANISM")
                    .ok()
                    .unwrap_or_else(|| "PLAIN".to_string()),
            );

            // send task to background
            tokio::spawn(async move {
                start_kafka_background_poll(
                    message_handler,
                    group_id,
                    brokers,
                    topics,
                    username,
                    password,
                    security_protocol,
                    sasl_mechanism,
                )
                .await
                .unwrap();
            });
        }

        Ok(())
    }
}
