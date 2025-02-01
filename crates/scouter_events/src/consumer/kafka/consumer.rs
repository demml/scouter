#[cfg(all(feature = "kafka", feature = "sql"))]
pub mod kafka_consumer {
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::CommitMode;
    use rdkafka::consumer::Consumer;
    use rdkafka::consumer::StreamConsumer;
    use rdkafka::message::BorrowedMessage;
    use rdkafka::message::Message;
    use scouter_error::EventError;
    use scouter_settings::KafkaSettings;
    use scouter_sql::MessageHandler;
    use scouter_types::ServerRecords;
    use std::collections::HashMap;
    use std::result::Result::Ok;
    use tracing::instrument;
    use tracing::Instrument;
    use tracing::{error, info, span, Level};

    // Get table name constant

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
            .set("enable.auto.commit", "true");

        if settings.username.is_some() && settings.password.is_some() {
            config
                .set("security.protocol", &settings.security_protocol)
                .set("sasl.mechanisms", &settings.sasl_mechanism)
                .set("sasl.username", settings.username.as_ref().unwrap())
                .set("sasl.password", settings.password.as_ref().unwrap());
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

    pub async fn stream_from_kafka_topic(
        message_handler: &MessageHandler,
        consumer: &StreamConsumer,
    ) -> Result<(), EventError> {
        loop {
            match consumer.recv().await {
                Err(e) => error!("Kafka error: {}", e),
                Ok(message) => {
                    if let Err(e) = process_kafka_message(message_handler, consumer, &message).await
                    {
                        error!("Error processing Kafka message: {:?}", e);
                    }
                }
            }
        }
    }

    pub async fn process_kafka_message(
        message_handler: &MessageHandler,
        consumer: &StreamConsumer,
        message: &BorrowedMessage<'_>,
    ) -> Result<(), EventError> {
        let payload = match message.payload_view::<str>() {
            None => {
                error!("No payload received");
                return Ok(());
            }
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                error!("Error while deserializing message payload: {:?}", e);
                return Ok(());
            }
        };

        let records: ServerRecords = match serde_json::from_str(payload) {
            Ok(records) => records,
            Err(e) => {
                error!("Failed to deserialize message: {:?}", e);
                return Ok(());
            }
        };

        match message_handler.insert_server_records(&records).await {
            Ok(_) => {
                consumer.commit_message(message, CommitMode::Async).unwrap();
            }
            Err(e) => {
                error!("Failed to insert drift record: {:?}", e);
            }
        }

        Ok(())
    }

    // Start background task to poll kafka topic
    //
    // This function will poll the kafka topic and insert the records into the database
    // using the provided message handler.
    //
    // # Arguments
    //
    // * `message_handler` - The message handler to process the records
    // * `group_id` - The kafka consumer group id
    // * `brokers` - The kafka brokers
    // * `topics` - The kafka topics to subscribe to
    // * `username` - The kafka username
    // * `password` - The kafka password
    // * `security_protocol` - The kafka security protocol
    // * `sasl_mechanism` - The kafka SASL mechanism
    //
    // # Returns
    //
    // * `Result<(), anyhow::Error>` - The result of the operation
    #[allow(clippy::unnecessary_unwrap)]
    #[allow(clippy::too_many_arguments)]
    pub async fn start_kafka_background_poll(
        message_handler: MessageHandler,
        settings: &KafkaSettings,
    ) -> Result<(), EventError> {
        let consumer = create_kafka_consumer(settings, None).await.unwrap();

        loop {
            if let Err(e) = stream_from_kafka_topic(&message_handler, &consumer)
                .instrument(span!(Level::INFO, "Kafka Consumer"))
                .await
            {
                error!("Error in stream_from_kafka_topic: {:?}", e);
            }
        }
    }
}
