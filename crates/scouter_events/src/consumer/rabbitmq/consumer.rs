#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub mod rabbitmq_consumer {

    use futures::StreamExt;
    use metrics::counter;
    use scouter_error::EventError;
    use scouter_settings::{DatabaseSettings, RabbitMQSettings};
    use scouter_sql::MessageHandler;
    use scouter_sql::PostgresClient;
    use scouter_types::ServerRecords;
    use sqlx::Pool;
    use sqlx::Postgres;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::task::JoinHandle;
    use tracing::{debug, error, info, instrument};

    use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties, Consumer};

    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    /// Create a new RabbitMQ consumer manager
    ///
    /// This function creates a new RabbitMQ consumer manager that will start a number of workers to consume messages from RabbitMQ
    pub struct RabbitMQConsumerManager {
        pub shutdown: Arc<AtomicBool>,
        pub workers: Vec<JoinHandle<()>>,
    }

    impl RabbitMQConsumerManager {
        /// Start a number of workers to consume messages from RabbitMQ
        ///
        /// This function creates a number of workers to consume messages from RabbitMQ. Each worker will consume messages from RabbitMQ and insert them into the database
        ///
        /// # Arguments
        ///
        /// * `rabbit_settings` - The RabbitMQ settings
        /// * `db_settings` - The database settings
        /// * `pool` - The database connection pool
        ///
        /// # Returns
        ///
        /// * `Result<RabbitMQConsumerManager, EventError>` - The result of the operation
        #[instrument(skip(rabbit_settings, db_settings, pool))]
        pub async fn start_workers(
            rabbit_settings: &RabbitMQSettings,
            db_settings: &DatabaseSettings,
            pool: &Pool<Postgres>,
        ) -> Result<Self, EventError> {
            let shutdown = Arc::new(AtomicBool::new(false));
            let num_consumers = rabbit_settings.num_consumers;
            let mut workers = Vec::with_capacity(num_consumers);

            for id in 0..num_consumers {
                let consumer = create_rabbitmq_consumer(rabbit_settings).await?;
                let kafka_db_client =
                    PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;
                let message_handler = MessageHandler::Postgres(kafka_db_client);

                workers.push(tokio::spawn(Self::start_worker(
                    id,
                    consumer,
                    message_handler,
                    shutdown.clone(),
                )));
            }

            debug!("✅ Started {} RabbitMQ workers", num_consumers);

            Ok(Self { shutdown, workers })
        }

        async fn start_worker(
            id: usize,
            mut consumer: Consumer,
            handler: MessageHandler,
            shutdown: Arc<AtomicBool>,
        ) {
            while !shutdown.load(Ordering::Relaxed) {
                while let Some(delivery) = consumer.next().await {
                    match delivery {
                        Ok(msg) => {
                            // check message size
                            if msg.data.len() > MAX_MESSAGE_SIZE {
                                error!("Message too large: {:?}", msg.data.len());
                                counter!("messages_too_large").increment(1);
                                continue;
                            }

                            // process messages. If processing fails, log the error, records metrics and continue
                            if let Ok(records) = process_message(&msg.data).await {
                                if let Some(records) = records {
                                    if let Err(e) = handler.insert_server_records(&records).await {
                                        error!(
                                            "Worker {}: Failed to insert drift record: {:?}",
                                            id, e
                                        );
                                        counter!("db_insert_errors").increment(1);
                                    } else {
                                        counter!("records_inserted")
                                            .increment(records.records.len() as u64);
                                        counter!("messages_processed").increment(1);

                                        // acknowledge the message. If acknowledgment fails, log the error
                                        if let Err(e) = msg.ack(BasicAckOptions::default()).await {
                                            error!("Failed to acknowledge message: {:?}", e);
                                            counter!("consumer_errors").increment(1);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Worker {}: RabbiteMQ error: {}", id, e);
                            counter!("consumer_errors").increment(1);
                        }
                    }
                }
            }
        }
    }

    pub async fn create_rabbitmq_consumer(
        settings: &RabbitMQSettings,
    ) -> Result<Consumer, EventError> {
        let conn = Connection::connect(&settings.address, ConnectionProperties::default())
            .await
            .map_err(|e| {
                error!("Failed to connect to RabbitMQ: {:?}", e);
                EventError::Error(format!("Failed to connect to RabbitMQ: {:?}", e))
            })?;
        let channel = conn.create_channel().await.unwrap();
        channel
            .basic_qos(settings.prefetch_count, BasicQosOptions::default())
            .await
            .map_err(|e| {
                error!("Failed to set QoS: {:?}", e);
                EventError::Error(format!("Failed to set QoS: {:?}", e))
            })?;

        channel
            .queue_declare(
                &settings.queue,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to declare queue: {:?}", e);
                EventError::Error(format!("Failed to declare queue: {:?}", e))
            })?;

        let consumer = channel
            .basic_consume(
                &settings.queue,
                &settings.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                error!("Failed to consume queue: {:?}", e);
                EventError::Error(format!("Failed to consume queue: {:?}", e))
            })?;

        info!("✅ Started consumer for RabbitMQ");

        Ok(consumer)
    }

    pub async fn process_message(message: &Vec<u8>) -> Result<Option<ServerRecords>, EventError> {
        let records: ServerRecords = match serde_json::from_slice::<ServerRecords>(&message) {
            Ok(records) => records,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(None);
            }
        };

        Ok(Some(records))
    }
}
