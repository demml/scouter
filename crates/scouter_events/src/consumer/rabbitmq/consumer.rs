#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub mod rabbitmq_consumer {
    use futures::StreamExt;
    use metrics::counter;
    use scouter_error::ScouterError;
    use scouter_settings::{DatabaseSettings, RabbitMQSettings};
    use scouter_sql::MessageHandler;
    use scouter_sql::PostgresClient;
    use scouter_types::ServerRecords;
    use sqlx::Pool;
    use sqlx::Postgres;
    use tokio::sync::watch;
    use tokio::task::JoinHandle;
    use tracing::{debug, error, info, instrument};

    use lapin::{
        message::Delivery, options::*, types::FieldTable, Connection, ConnectionProperties,
        Consumer,
    };

    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    /// Create a new RabbitMQ consumer manager
    ///
    /// This function creates a new RabbitMQ consumer manager that will start a number of workers to consume messages from RabbitMQ
    pub struct RabbitMQConsumerManager {
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
        #[instrument(
            skip(rabbit_settings, db_settings, pool, shutdown_rx),
            name = "start_rabbitmq_workers"
        )]
        pub async fn start_workers(
            rabbit_settings: &RabbitMQSettings,
            db_settings: &DatabaseSettings,
            pool: &Pool<Postgres>,
            shutdown_rx: watch::Receiver<()>,
        ) -> Result<Self, ScouterError> {
            let num_consumers = rabbit_settings.num_consumers;
            let mut workers = Vec::with_capacity(num_consumers);

            for id in 0..num_consumers {
                let consumer = create_rabbitmq_consumer(rabbit_settings).await?;
                let kafka_db_client =
                    PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;
                let message_handler = MessageHandler::Postgres(kafka_db_client);

                let worker_shutdown_rx = shutdown_rx.clone();
                workers.push(tokio::spawn(async move {
                    Self::start_worker(id, consumer, message_handler, worker_shutdown_rx).await;
                }));
            }

            debug!("✅ Started {} RabbitMQ workers", num_consumers);

            Ok(Self { workers })
        }

        async fn start_worker(
            id: usize,
            mut consumer: Consumer,
            handler: MessageHandler,
            mut shutdown: watch::Receiver<()>, // Accept receiver
        ) {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        info!("RabbitMQ worker {}: Shutting down", id);
                        break;
                    }
                    delivery = consumer.next() => {
                        match delivery {
                            Some(Ok(msg)) => {
                                handle_message(id, msg, &handler).await;
                            }
                            Some(Err(e)) => {
                                error!("Worker {}: RabbitMQ error: {}", id, e);
                                counter!("consumer_errors").increment(1);
                            }
                            None => {
                                info!("Worker {}: No more messages", id);
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_message(id: usize, msg: Delivery, handler: &MessageHandler) {
        // Check message size
        if msg.data.len() > MAX_MESSAGE_SIZE {
            error!("Worker {}: Message too large: {:?}", id, msg.data.len());
            counter!("messages_too_large").increment(1);
            return;
        }

        // Process messages. If processing fails, log the error, record metrics, and continue
        match process_message(&msg.data).await {
            Ok(Some(records)) => {
                if let Err(e) = handler.insert_server_records(&records).await {
                    error!("Worker {}: Failed to insert drift record: {:?}", id, e);
                    counter!("db_insert_errors").increment(1);
                } else {
                    counter!("records_inserted").increment(records.records.len() as u64);
                    counter!("messages_processed").increment(1);

                    // Acknowledge the message. If acknowledgment fails, log the error
                    if let Err(e) = msg.ack(BasicAckOptions::default()).await {
                        error!("Worker {}: Failed to acknowledge message: {:?}", id, e);
                        counter!("consumer_errors").increment(1);
                    }
                }
            }
            Ok(None) => {
                // No records to process
            }
            Err(e) => {
                error!("Worker {}: Error processing message: {:?}", id, e);
                counter!("message_processing_errors").increment(1);
            }
        }
    }

    pub async fn create_rabbitmq_consumer(
        settings: &RabbitMQSettings,
    ) -> Result<Consumer, ScouterError> {
        let conn = Connection::connect(&settings.address, ConnectionProperties::default())
            .await
            .map_err(ScouterError::traced_connect_rabbitmq_error)?;

        let channel = conn.create_channel().await.unwrap();
        channel
            .basic_qos(settings.prefetch_count, BasicQosOptions::default())
            .await
            .map_err(ScouterError::traced_setup_qos_error)?;

        channel
            .queue_declare(
                &settings.queue,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(ScouterError::traced_declare_queue_error)?;

        let consumer = channel
            .basic_consume(
                &settings.queue,
                &settings.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(ScouterError::traced_consume_queue_error)?;

        info!("✅ Started consumer for RabbitMQ");

        Ok(consumer)
    }

    pub async fn process_message(message: &[u8]) -> Result<Option<ServerRecords>, ScouterError> {
        let records: ServerRecords = match serde_json::from_slice::<ServerRecords>(message) {
            Ok(records) => records,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(None);
            }
        };

        Ok(Some(records))
    }
}
