#[cfg(feature = "rabbitmq")]
pub mod rabbitmq_startup {
    use crate::consumer::rabbitmq::consumer::rabbitmq_consumer::start_rabbitmq_background_poll;
    use anyhow::*;
    use scouter_settings::ScouterServerConfig;
    use scouter_sql::{MessageHandler, PostgresClient};
    use sqlx::{Pool, Postgres};
    use std::result::Result;
    use tracing::info;

    pub async fn startup_rabbitmq(
        pool: &Pool<Postgres>,
        config: &ScouterServerConfig,
    ) -> Result<(), anyhow::Error> {
        info!("Starting RabbitMQ consumer");

        let rabbit_settings = config.rabbitmq_settings.as_ref().unwrap().clone();
        let database_settings = &config.database_settings;
        let num_consumers = rabbit_settings.num_consumers.clone();

        for _ in 0..num_consumers {
            let rabbit_db_client = PostgresClient::new(Some(pool.clone()), Some(database_settings))
                .await
                .with_context(|| "Failed to create Postgres client")?;
            let message_handler = MessageHandler::Postgres(rabbit_db_client);
            let address = rabbit_settings.address.clone();
            let prefetch_count = rabbit_settings.prefetch_count.clone();

            tokio::spawn(async move {
                start_rabbitmq_background_poll(message_handler, address, prefetch_count)
                    .await
                    .unwrap();
            });
        }

        Ok(())
    }
}
