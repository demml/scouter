#[cfg(feature = "kafka")]
pub mod kafka_consumer {
    use scouter_sql::MessageHandler;

    use anyhow::*;
    use rdkafka::config::ClientConfig;
    use rdkafka::consumer::CommitMode;
    use rdkafka::consumer::Consumer;
    use rdkafka::consumer::StreamConsumer;
    use rdkafka::message::BorrowedMessage;
    use rdkafka::message::Message;
    use scouter_types::ServerRecords;
    use std::collections::HashMap;
    use std::result::Result::Ok;
    use tracing::error;
    use tracing::info;

    // Get table name constant

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unnecessary_unwrap)]
    pub async fn create_kafka_consumer(
        group_id: String,
        brokers: String,
        topics: Vec<String>,
        username: Option<String>,
        password: Option<String>,
        security_protocol: String,
        sasl_mechanism: String,
        config_overrides: Option<HashMap<&str, &str>>,
    ) -> Result<StreamConsumer, anyhow::Error> {
        let mut config = ClientConfig::new();

        config
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("enable.partition.eof", "false")
            .set("session.timeout.ms", "6000")
            .set("enable.auto.commit", "true");

        if username.is_some() && password.is_some() {
            config
                .set("security.protocol", security_protocol)
                .set("sasl.mechanisms", sasl_mechanism)
                .set("sasl.username", username.unwrap())
                .set("sasl.password", password.unwrap());
        }

        if let Some(overrides) = config_overrides {
            for (key, value) in overrides {
                config.set(key, value);
            }
        }

        let consumer: StreamConsumer = config.create().expect("Consumer creation error");

        let topics = topics.iter().map(|s| s.as_str()).collect::<Vec<&str>>();

        consumer
            .subscribe(&topics)
            .expect("Can't subscribe to specified topics");

        info!("✅ Started consumer for topics: {:?}", topics);
        Ok(consumer)
    }

    pub async fn stream_from_kafka_topic(
        message_handler: &MessageHandler,
        consumer: &StreamConsumer,
    ) -> Result<()> {
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

    async fn process_kafka_message(
        message_handler: &MessageHandler,
        consumer: &StreamConsumer,
        message: &BorrowedMessage<'_>,
    ) -> Result<()> {
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
        group_id: String,
        brokers: String,
        topics: Vec<String>,
        username: Option<String>,
        password: Option<String>,
        security_protocol: String,
        sasl_mechanism: String,
    ) -> Result<(), anyhow::Error> {
        let consumer = create_kafka_consumer(
            group_id,
            brokers,
            topics,
            username,
            password,
            security_protocol,
            sasl_mechanism,
            None,
        )
        .await
        .unwrap();

        loop {
            if let Err(e) = stream_from_kafka_topic(&message_handler, &consumer).await {
                error!("Error in stream_from_kafka_topic: {:?}", e);
            }
        }
    }
}
