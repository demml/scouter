pub mod kafka_consumer {
    use crate::error::EventError;
    use metrics::counter;
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::CommitMode;
    use rdkafka::consumer::Consumer;
    use rdkafka::consumer::StreamConsumer;
    use rdkafka::message::BorrowedMessage;
    use rdkafka::message::Message;
    use scouter_settings::KafkaSettings;
    use scouter_sql::MessageHandler;
    use scouter_types::MessageRecord;
    use sqlx::Pool;
    use sqlx::Postgres;
    use std::collections::HashMap;
    use std::result::Result::Ok;
    use tokio::sync::watch;
    use tokio::task::JoinHandle;
    use tracing::instrument;
    use tracing::{error, info};

    const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB

    pub struct KafkaConsumerManager {
        pub workers: Vec<JoinHandle<()>>,
    }

    impl KafkaConsumerManager {
        pub async fn start_worker(
            id: usize,
            consumer: StreamConsumer,
            db_pool: Pool<Postgres>,
            mut shutdown: watch::Receiver<()>,
        ) {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        info!("Kafka worker {}: Shutting down", id);
                        break;
                    }
                    msg = consumer.recv() => {
                        match msg {
                            Ok(msg) => {
                                if msg.payload_len() > MAX_MESSAGE_SIZE {
                                    error!("Worker {}: Message too large", id);
                                    counter!("messages_too_large").increment(1);
                                    continue;
                                }

                                if let Ok(Some(record)) = process_message(&msg).await {
                                    let result = match record {
                                        MessageRecord::ServerRecords(records) => {
                                            MessageHandler::insert_server_records(&db_pool, &records).await
                                        }
                                        MessageRecord::TraceServerRecord(trace_record) => {
                                            MessageHandler::insert_trace_server_record(&db_pool, &trace_record).await

                                        }
                                        MessageRecord::TagServerRecord(tag_record) => {
                                            MessageHandler::insert_tag_record(&db_pool, &tag_record).await
                                        }
                                    };

                                    if let Err(e) = result {
                                        error!("Worker {}: Error handling message: {}", id, e);
                                        counter!("db_insert_errors").increment(1);
                                    } else {
                                        counter!("messages_processed").increment(1);
                                        consumer
                                            .commit_message(&msg, CommitMode::Async)
                                            .map_err(|e| {
                                                error!("Worker {}: Failed to commit message: {}", id, e);
                                                counter!("consumer_errors").increment(1);
                                            })
                                            .unwrap_or(());
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
                .set("sasl.mechanism", &settings.sasl_mechanism)
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

        let consumer: StreamConsumer = config
            .create()
            .map_err(EventError::ConnectKafkaConsumerError)?;

        let topics = settings
            .topics
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>();

        consumer
            .subscribe(&topics)
            .map_err(EventError::SubscribeTopicError)?;

        info!("✅ Started consumer for topics: {:?}", topics);
        Ok(consumer)
    }

    pub async fn process_message(
        message: &BorrowedMessage<'_>,
    ) -> Result<Option<MessageRecord>, EventError> {
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

        let record: MessageRecord = match serde_json::from_str(payload) {
            Ok(record) => record,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(None);
            }
        };

        Ok(Some(record))
    }
}
