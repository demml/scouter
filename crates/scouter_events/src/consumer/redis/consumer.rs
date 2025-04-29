#[cfg(all(feature = "redis_events", feature = "sql"))]
pub mod redis_consumer {
    use crate::producer::redis::producer::redis_producer::RedisMessageBroker;
    use futures_util::StreamExt;
    use metrics::counter;
    use redis::aio::PubSub;
    use redis::{Msg, RedisResult};
    use scouter_error::EventError;
    use scouter_settings::RedisSettings;
    use scouter_sql::MessageHandler;
    use scouter_types::ServerRecords;
    use sqlx::{Pool, Postgres};
    use tokio::sync::watch;
    use tokio::task::JoinHandle;
    use tracing::{debug, error, info, instrument};

    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    pub struct RedisConsumer {
        pub sub: PubSub,
    }

    impl RedisConsumer {
        pub async fn new(broker: &RedisMessageBroker) -> Result<Self, EventError> {
            let sub = broker.get_pub_sub().await?;
            Ok(Self { sub })
        }

        pub async fn subscribe(&mut self, channel: &str) -> Result<(), EventError> {
            self.sub
                .subscribe(channel)
                .await
                .map_err(|e| EventError::SubscribeError(e.to_string()))
        }
    }

    /// Create a new RabbitMQ consumer manager
    ///
    /// This function creates a new RabbitMQ consumer manager that will start a number of workers to consume messages from RabbitMQ
    pub struct RedisConsumerManager {
        pub workers: Vec<JoinHandle<()>>,
    }

    impl RedisConsumerManager {
        /// Start a number of workers to consume messages from Redis
        /// This function creates a number of workers to consume messages from Redis.
        /// Each worker will consume messages from Redis and insert them into the database
        ///
        /// # Arguments
        /// * `redis_settings` - The Redis settings
        /// * `db_pool` - The database client
        /// * `shutdown_rx` - The shutdown receiver
        ///
        /// # Returns
        ///
        /// * `Result<RedisConsumerManager, EventError>` - The result of the operation
        #[instrument(skip_all, name = "start_redis_workers")]
        pub async fn start_workers(
            redis_settings: &RedisSettings,
            db_pool: &Pool<Postgres>,
            shutdown_rx: watch::Receiver<()>,
        ) -> Result<Self, EventError> {
            let num_consumers = redis_settings.num_consumers;
            let mut workers = Vec::with_capacity(num_consumers);

            for id in 0..num_consumers {
                let consumer = create_redis_consumer(redis_settings).await?;
                let redis_db_pool = db_pool.clone();
                let worker_shutdown_rx = shutdown_rx.clone();

                workers.push(tokio::spawn(async move {
                    Self::start_worker(id, consumer, redis_db_pool, worker_shutdown_rx).await;
                }));
            }

            debug!("✅ Started {} Redis workers", num_consumers);

            Ok(Self { workers })
        }

        async fn start_worker(
            id: usize,
            consumer: RedisConsumer,
            db_pool: Pool<Postgres>,
            mut shutdown: watch::Receiver<()>, // Accept receiver
        ) {
            // Convert PubSub into a stream of messages
            let mut message_stream = consumer.sub.into_on_message();

            loop {
                tokio::select! {
                    // Branch for shutdown signal
                    _ = shutdown.changed() => {
                        info!("Redis worker {}: Shutting down", id);
                        break;
                    }

                    // Branch for receiving messages from Redis
                    maybe_msg = message_stream.next() => {
                        match maybe_msg {
                            Some(msg) => {
                                handle_message(id, msg, &db_pool).await;
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

    async fn handle_message(id: usize, msg: Msg, db_pool: &Pool<Postgres>) {
        // Check message size
        let payload: RedisResult<String> = msg.get_payload();

        if payload.is_err() {
            error!("Worker {}: Failed to get payload: {:?}", id, payload);
            counter!("consumer_errors").increment(1);
            return;
        }

        // Unwrap the payload
        let payload = payload.unwrap();

        if payload.len() > MAX_MESSAGE_SIZE {
            error!("Worker {}: Message too large: {:?}", id, payload.len());
            counter!("messages_too_large").increment(1);
            return;
        }

        // Process messages. If processing fails, log the error, record metrics, and continue
        match process_message(&payload).await {
            Ok(Some(records)) => {
                if let Err(e) = MessageHandler::insert_server_records(db_pool, &records).await {
                    error!("Worker {}: Failed to insert drift record: {:?}", id, e);
                    counter!("db_insert_errors").increment(1);
                } else {
                    counter!("records_inserted").increment(records.records.len() as u64);
                    counter!("messages_processed").increment(1);
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

    /// Extract the server records from the message
    pub async fn process_message(message: &str) -> Result<Option<ServerRecords>, EventError> {
        let records: ServerRecords = match serde_json::from_str::<ServerRecords>(message) {
            Ok(records) => records,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(None);
            }
        };

        Ok(Some(records))
    }

    /// Create a new Redis consumer
    pub async fn create_redis_consumer(
        settings: &RedisSettings,
    ) -> Result<RedisConsumer, EventError> {
        let broker = RedisMessageBroker::new(&settings.address)?;
        let mut consumer = RedisConsumer::new(&broker).await?;
        consumer.subscribe(&settings.channel).await?;
        Ok(consumer)
    }
}
