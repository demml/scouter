pub mod kafka_consumer {
    use metrics::counter;
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::CommitMode;
    use rdkafka::consumer::Consumer;
    use rdkafka::consumer::StreamConsumer;
    use rdkafka::message::BorrowedMessage;
    use rdkafka::message::Message;
    use scouter_error::EventError;
    use scouter_settings::{DatabaseSettings, KafkaSettings};
    use scouter_sql::MessageHandler;
    use scouter_sql::PostgresClient;
    use scouter_types::ServerRecords;
    use sqlx::Pool;
    use sqlx::Postgres;
    use std::collections::HashMap;
    use std::result::Result::Ok;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::task::JoinHandle;
    use tracing::instrument;
    use tracing::{error, info};

    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    pub struct KafkaConsumerManager {
        shutdown: Arc<AtomicBool>,
        workers: Vec<JoinHandle<()>>,
    }

    impl KafkaConsumerManager {
        pub async fn start_workers(
            kafka_settings: &KafkaSettings,
            db_settings: &DatabaseSettings,
            pool: &Pool<Postgres>,
        ) -> Result<Self, EventError> {
            let shutdown = Arc::new(AtomicBool::new(false));
            let num_consumers = kafka_settings.num_workers;
            let mut workers = Vec::with_capacity(num_consumers);

            for id in 0..num_consumers {
                let consumer = create_kafka_consumer(kafka_settings, None).await?;
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

            Ok(Self { shutdown, workers })
        }

        async fn start_worker(
            id: usize,
            consumer: StreamConsumer,
            handler: MessageHandler,
            shutdown: Arc<AtomicBool>,
        ) {
            while !shutdown.load(Ordering::Relaxed) {
                match consumer.recv().await {
                    Ok(msg) => {
                        if msg.payload_len() > MAX_MESSAGE_SIZE {
                            error!("Worker {}: Message too large", id);
                            counter!("messages_too_large").increment(1);
                            continue;
                        }

                        if let Ok(records) = process_message(&msg).await {
                            if let Some(records) = records {
                                if let Err(e) = handler.insert_server_records(&records).await {
                                    error!("Worker {}: Error handling message: {}", id, e);
                                    counter!("db_insert_errors").increment(1);
                                } else {
                                    counter!("records_inserted")
                                        .absolute(records.records.len() as u64);
                                    counter!("messages_processed").increment(1);
                                    consumer
                                        .commit_message(&msg, CommitMode::Async)
                                        .map_err(|e| {
                                            error!(
                                                "Worker {}: Failed to commit message: {}",
                                                id, e
                                            );
                                            counter!("consumer_errors").increment(1);
                                        })
                                        .unwrap_or(());
                                }
                            }
                        }
                    }

                    Err(e) => {
                        error!("Worker {}: Kafka error: {}", id, e);
                        counter!("consumer_errors").increment(1);
                    }
                }
            }
        }

        pub async fn shutdown(&self) {
            self.shutdown.store(true, Ordering::Relaxed);
            for worker in &self.workers {
                worker.abort();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unnecessary_unwrap)]
    #[instrument(skip(settings, config_overrides))]
    pub async fn create_kafka_consumer(
        settings: &KafkaSettings,
        config_overrides: Option<HashMap<&str, &str>>,
    ) -> Result<StreamConsumer, EventError> {
        let mut config = ClientConfig::new();

        config
            .set("group.id", &settings.group_id)
            .set("bootstrap.servers", &settings.brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("auto.offset.reset", &settings.offset_reset)
            .set("enable.auto.commit", "true");

        if settings.username.is_some() && settings.password.is_some() {
            config
                .set("security.protocol", &settings.security_protocol)
                .set("sasl.mechanisms", &settings.sasl_mechanism)
                .set("sasl.username", settings.username.as_ref().unwrap())
                .set("sasl.password", settings.password.as_ref().unwrap());

            if let Some(cert_location) = &settings.cert_location {
                config.set("ssl.ca.location", cert_location);
            }
        }

        if let Some(overrides) = config_overrides {
            for (key, value) in overrides {
                config.set(key, value);
            }
        }

        let consumer: StreamConsumer = config.create().map_err(|e| {
            error!("Failed to create Kafka consumer: {:?}", e);
            EventError::Error(format!("Failed to create Kafka consumer: {:?}", e))
        })?;

        let topics = settings
            .topics
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>();

        consumer.subscribe(&topics).map_err(|e| {
            error!("Failed to subscribe to topics: {:?}", e);
            EventError::Error(format!("Failed to subscribe to topics: {:?}", e))
        })?;

        info!("✅ Started consumer for topics: {:?}", topics);
        Ok(consumer)
    }

    pub async fn process_message(
        message: &BorrowedMessage<'_>,
    ) -> Result<Option<ServerRecords>, EventError> {
        let payload = match message.payload_view::<str>() {
            None => {
                error!("No payload received");
                return Ok(None);
            }
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                error!("Error while deserializing message payload: {:?}", e);
                return Ok(None);
            }
        };

        let records: ServerRecords = match serde_json::from_str(payload) {
            Ok(records) => records,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(None);
            }
        };

        Ok(Some(records))
    }
}
