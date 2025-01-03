#[cfg(feature = "kafka")]
pub mod kafka_startup {

    use crate::consumer::kafka::consumer::kafka_consumer::start_kafka_background_poll;
    use scouter_error::EventError;
    use scouter_settings::ScouterServerConfig;
    use scouter_sql::{MessageHandler, PostgresClient};
    use sqlx::{Pool, Postgres};
    use tracing::info;

    pub async fn startup_kafka(
        pool: &Pool<Postgres>,
        config: &ScouterServerConfig,
    ) -> Result<(), EventError> {
        info!("Starting Kafka consumer");

        let kafka_settings = config.kafka_settings.as_ref().unwrap().clone();
        let database_settings = &config.database_settings;
        let num_consumers = kafka_settings.num_workers.clone();

        for _ in 0..num_consumers {
            let kafka_db_client =
                PostgresClient::new(Some(pool.clone()), Some(database_settings)).await?;
            let message_handler = MessageHandler::Postgres(kafka_db_client);
            let settings = kafka_settings.clone();

            // send task to background
            tokio::spawn(async move {
                start_kafka_background_poll(message_handler, &settings)
                    .await
                    .unwrap();
            });
        }

        Ok(())
    }
}
