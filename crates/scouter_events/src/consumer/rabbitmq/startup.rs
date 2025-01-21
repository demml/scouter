#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub mod rabbitmq_startup {
    use crate::consumer::rabbitmq::consumer::rabbitmq_consumer::start_rabbitmq_background_poll;
    use scouter_error::EventError;
    use scouter_settings::ScouterServerConfig;
    use scouter_sql::{MessageHandler, PostgresClient};
    use sqlx::{Pool, Postgres};
    use std::result::Result;
    use tracing::info;

    pub async fn startup_rabbitmq(
        pool: &Pool<Postgres>,
        config: &ScouterServerConfig,
    ) -> Result<(), EventError> {
        info!("Starting RabbitMQ consumer");

        let rabbit_settings = config.rabbitmq_settings.as_ref().unwrap().clone();
        let database_settings = &config.database_settings;
        let num_consumers = rabbit_settings.num_consumers;

        for _ in 0..num_consumers {
            let rabbit_db_client =
                PostgresClient::new(Some(pool.clone()), Some(database_settings)).await?;
            let message_handler = MessageHandler::Postgres(rabbit_db_client);
            let settings = rabbit_settings.clone();

            tokio::spawn(async move {
                start_rabbitmq_background_poll(message_handler, &settings)
                    .await
                    .unwrap();
            });
        }

        Ok(())
    }
}
