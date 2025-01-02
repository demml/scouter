#[cfg(feature = "kafka")]
pub mod kafka_startup {

    use crate::consumer::kafka::consumer::kafka_consumer::start_kafka_background_poll;
    use anyhow::*;
    use scouter_settings::ScouterServerConfig;
    use scouter_sql::{MessageHandler, PostgresClient};
    use sqlx::{Pool, Postgres};
    use tracing::info;

    pub async fn startup_kafka(pool: &Pool<Postgres>, config: &ScouterServerConfig) -> Result<()> {
        info!("Starting Kafka consumer");

        let kafka_settings = config.kafka_settings.as_ref().unwrap().clone();
        let database_settings = &config.database_settings;
        let num_consumers = kafka_settings.num_workers.clone();

        for _ in 0..num_consumers {
            let kafka_db_client = PostgresClient::new(Some(pool.clone()), Some(database_settings))
                .await
                .with_context(|| "Failed to create Postgres client")?;
            let message_handler = MessageHandler::Postgres(kafka_db_client);

            let group_id = kafka_settings.group_id.clone();
            let brokers = kafka_settings.brokers.clone();
            let topics = kafka_settings.topics.clone();
            let username = kafka_settings.username.clone();
            let password = kafka_settings.password.clone();
            let security_protocol = kafka_settings.security_protocol.clone();
            let sasl_mechanism = kafka_settings.sasl_mechanism.clone();

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
