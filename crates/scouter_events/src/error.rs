use futures::io;
use pyo3::PyErr;
use thiserror::Error;

#[cfg(feature = "kafka")]
use rdkafka::error::KafkaError;

use crate::queue::bus::Event;

#[derive(Error, Debug)]
pub enum FeatureQueueError {
    #[error("{0}")]
    InvalidFormatError(String),

    #[error("Failed to create drift record: {0}")]
    DriftRecordError(String),

    #[error("Failed to create alert record: {0}")]
    AlertRecordError(String),

    #[error("Failed to get feature")]
    GetFeatureError,

    #[error("Missing feature map")]
    MissingFeatureMapError,

    #[error("invalid data type detected for feature: {0}")]
    InvalidFeatureTypeError(String),

    #[error("invalid value detected for feature: {0}, error: {1}")]
    InvalidValueError(String, String),

    #[error("Failed to get bin given bin id")]
    GetBinError,
}

impl From<FeatureQueueError> for PyErr {
    fn from(err: FeatureQueueError) -> PyErr {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
    }
}

#[derive(Error, Debug)]
pub enum EventError {
    #[cfg(feature = "kafka")]
    #[error("Failed to connect to kakfa consumer")]
    ConnectKafkaConsumerError(#[source] KafkaError),

    #[cfg(feature = "kafka")]
    #[error("Failed to connect to kakfa producer")]
    ConnectKafkaProducerError(#[source] KafkaError),

    #[cfg(feature = "kafka")]
    #[error("Failed to subscribe to topic")]
    SubscribeTopicError(#[source] KafkaError),

    #[cfg(feature = "kafka")]
    #[error("Failed to flush kafka producer")]
    FlushKafkaProducerError(#[source] KafkaError),

    #[cfg(feature = "kafka")]
    #[error("Failed to create producer")]
    CreateKafkaProducerError(#[source] KafkaError),

    #[cfg(feature = "kafka")]
    #[error("Failed to publish message")]
    PublishKafkaMessageError(#[source] KafkaError),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to connect to RabbitMQ")]
    ConnectRabbitMQError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to setup RabbitMQ QoS")]
    SetupRabbitMQQosError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to declare RabbitMQ queue")]
    DeclareRabbitMQQueueError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to consume RabbitMQ queue")]
    ConsumeRabbitMQError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to create RabbitMQ channel")]
    CreateRabbitMQChannelError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to publish RabbitMQ message")]
    PublishRabbitMQMessageError(#[source] lapin::Error),

    #[cfg(feature = "rabbitmq")]
    #[error("Failed to flush RabbitMQ channel")]
    FlushRabbitMQChannelError(#[source] lapin::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    HeaderError(#[from] reqwest::header::InvalidHeaderValue),

    #[error("Unauthorized")]
    UnauthorizedError,

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    SendEntityError(#[from] tokio::sync::mpsc::error::SendError<Event>),

    #[error("Failed to push to queue. Queue is full")]
    QueuePushError,

    #[error("Failed to push to queue. Max retries exceeded")]
    QueuePushRetryError,

    #[error("Queue not supported for feature entity")]
    QueueNotSupportedFeatureError,

    #[error("Queue not supported for metrics entity")]
    QueueNotSupportedMetricsError,

    #[error("Failed to signal startup")]
    SignalStartupError,

    #[error("Failed to signal startup")]
    SignalCompletionError,

    #[error("Failed to setup tokio runtime")]
    SetupTokioRuntimeError(#[source] io::Error),

    #[error("Failed to setup tokio runtime")]
    StartupReceiverError(#[source] tokio::sync::oneshot::error::RecvError),

    #[error("Failed to setup tokio runtime")]
    ShutdownReceiverError(#[source] tokio::sync::oneshot::error::RecvError),

    #[error("Kafka feature not enabled")]
    KafkaFeatureNotEnabledError,

    #[error("RabbitMQ feature not enabled")]
    RabbitMQFeatureNotEnabledError,

    #[error("Invalid compressions type")]
    InvalidCompressionTypeError,
}

#[derive(Error, Debug)]
pub enum PyEventError {
    #[error(transparent)]
    EventError(#[from] EventError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Invalid compressions type")]
    InvalidCompressionTypeError,

    #[error(transparent)]
    TypeError(#[from] scouter_types::error::TypeError),

    #[error(transparent)]
    ProfileError(#[from] scouter_types::error::ProfileError),

    #[error("Failed to get queue: {0}")]
    MissingQueueError(String),

    #[error("Failed to shutdown queue")]
    ShutdownQueueError(#[source] pyo3::PyErr),
}
impl From<PyEventError> for PyErr {
    fn from(err: PyEventError) -> PyErr {
        let msg = err.to_string();
        pyo3::exceptions::PyRuntimeError::new_err(msg)
    }
}
