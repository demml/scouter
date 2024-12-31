#[cfg(feature = "rabbitmq")]
pub mod rabbitmq_consumer {

    use crate::consumer::base::MessageHandler;
    use scouter::core::drift::base::ServerRecords;

    use futures::StreamExt;

    use std::result::Result::Ok;
    use tracing::error;
    use tracing::info;

    use lapin::{
        options::*, types::FieldTable, Connection, ConnectionProperties, Consumer, Result,
    };

    pub async fn create_rabbitmq_consumer(address: &str, prefetch_count: &u16) -> Result<Consumer> {
        let conn = Connection::connect(address, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await.unwrap();
        channel
            .basic_qos(*prefetch_count, BasicQosOptions::default())
            .await?;

        channel
            .queue_declare(
                "scouter_monitoring",
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let consumer = channel
            .basic_consume(
                "scouter_monitoring",
                "scouter_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        info!("✅ Started consumer for RabbitMQ");

        Ok(consumer)
    }

    pub async fn stream_from_rabbit_queue(
        message_handler: &MessageHandler,
        consumer: &mut Consumer,
    ) -> Result<()> {
        // start consumer. The `start` method returns a future that resolves to a `Result` once the consumer has started.
        while let Some(delivery) = consumer.next().await {
            match delivery {
                // try loading the message from the delivery. if message serialization fails, log the error
                Ok(delivery) => match serde_json::from_slice::<ServerRecords>(&delivery.data) {
                    // insert the records into the database. If insertion fails, log the error
                    Ok(records) => match message_handler.insert_server_records(&records).await {
                        // acknowledge the message. If acknowledgment fails, log the error
                        Ok(_) => {
                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!("Failed to acknowledge message: {:?}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to insert drift record: {:?}", e);
                        }
                    },
                    Err(e) => {
                        error!("Failed to deserialize message: {:?}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to receive delivery: {:?}", e);
                }
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

    pub async fn start_rabbitmq_background_poll(
        message_handler: MessageHandler,
        address: String,
        prefetch_count: u16,
    ) -> Result<()> {
        let mut consumer = create_rabbitmq_consumer(&address, &prefetch_count).await?;

        loop {
            if let Err(e) = stream_from_rabbit_queue(&message_handler, &mut consumer).await {
                error!("Error in stream_from_rabbit_queue: {:?}", e);
            }
        }
    }
}
