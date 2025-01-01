#[cfg(feature = "rabbitmq")]
pub mod rabbitmq_startup {
    use scouter_sql::{PostgresClient, MessageHandler};
    use crate::consumer::rabbitmq::consumer::rabbitmq_consumer::start_rabbitmq_background_poll;
    use sqlx::{Pool, Postgres};
    use std::result::Result;
    use tracing::info;
    use anyhow::*;

    pub async fn startup_rabbitmq(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
        info!("Starting RabbitMQ consumer");

        let num_rabbits = std::env::var("RABBITMQ_CONSUMERS_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .map_err(|e| lapin::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let prefetch_count = std::env::var("RABBITMQ_PREFETCH_COUNT")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u16>()
            .map_err(|e| lapin::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        for _ in 0..num_rabbits {
            let rabbit_db_client = PostgresClient::new(Some(pool.clone())).await
            .with_context(|| "Failed to create Postgres client")?;
            let message_handler = MessageHandler::Postgres(rabbit_db_client);

            let rabbit_addr = std::env::var("RABBITMQ_ADDR")
                .unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5672/%2f".into());

            tokio::spawn(async move {
                start_rabbitmq_background_poll(message_handler, rabbit_addr, prefetch_count)
                    .await
                    .unwrap();
            });
        }

        Ok(())
    }
}
