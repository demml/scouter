use pyo3::PyErr;
use thiserror::Error;

#[cfg(feature = "kafka")]
use rdkafka::error::KafkaError;

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
}

#[derive(Error, Debug)]
pub enum PyEventError {
    #[error(transparent)]
    EventError(#[from] EventError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Invalid compressions type")]
    InvalidCompressionTypeError,
}
impl From<PyEventError> for PyErr {
    fn from(err: PyEventError) -> PyErr {
        let msg = err.to_string();
        pyo3::exceptions::PyRuntimeError::new_err(msg)
    }
}
