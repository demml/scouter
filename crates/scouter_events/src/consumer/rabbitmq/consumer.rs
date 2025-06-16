#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub mod rabbitmq_consumer {
    use crate::error::EventError;
    use futures::StreamExt;
    use metrics::counter;
    use scouter_settings::RabbitMQSettings;
    use scouter_sql::MessageHandler;
    use scouter_types::ServerRecords;
    use sqlx::{Pool, Postgres};
    use tokio::sync::watch;
    use tokio::task::JoinHandle;
    use tracing::{error, info, instrument};

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
        #[instrument(skip_all, name = "start_rabbitmq_worker")]
        pub async fn start_worker(
            id: usize,
            mut consumer: Consumer,
            db_pool: Pool<Postgres>,
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
                                handle_message(id, msg, &db_pool).await;
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

    async fn handle_message(id: usize, msg: Delivery, db_pool: &Pool<Postgres>) {
        // Check message size
        if msg.data.len() > MAX_MESSAGE_SIZE {
            error!("Worker {}: Message too large: {:?}", id, msg.data.len());
            counter!("messages_too_large").increment(1);
            return;
        }

        // Process messages. If processing fails, log the error, record metrics, and continue
        match process_message(&msg.data).await {
            Ok(Some(records)) => {
                if let Err(e) = MessageHandler::insert_server_records(db_pool, &records).await {
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
    ) -> Result<Consumer, EventError> {
        let conn = Connection::connect(&settings.address, ConnectionProperties::default())
            .await
            .map_err(EventError::ConnectRabbitMQError)?;

        let channel = conn.create_channel().await.unwrap();
        channel
            .basic_qos(settings.prefetch_count, BasicQosOptions::default())
            .await
            .map_err(EventError::SetupRabbitMQQosError)?;

        channel
            .queue_declare(
                &settings.queue,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(EventError::DeclareRabbitMQQueueError)?;

        let consumer = channel
            .basic_consume(
                &settings.queue,
                &settings.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(EventError::ConsumeRabbitMQError)?;

        info!("✅ Started consumer for RabbitMQ");

        Ok(consumer)
    }

    pub async fn process_message(message: &[u8]) -> Result<Option<ServerRecords>, EventError> {
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
