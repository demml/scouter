use pyo3::prelude::*;

#[pyclass]
#[derive(Clone)]
pub struct RabbitMQConfig {
    #[pyo3(get, set)]
    pub address: String,

    #[pyo3(get, set)]
    pub queue: String,

    #[pyo3(get, set)]
    pub raise_on_error: bool,
}

#[pymethods]
impl RabbitMQConfig {
    #[new]
    #[pyo3(signature = (address=None, queue="scouter_monitoring".to_string(), raise_on_error=false))]
    pub fn new(
        address: Option<String>,
        queue: Option<String>,
        raise_on_error: Option<bool>,
    ) -> Self {
        let address = address.unwrap_or_else(|| {
            std::env::var("RABBITMQ_ADDRESS")
                .unwrap_or_else(|_| "amqp://guest:guest@127.0.0.1:5672/%2f".to_string())
        });
        let queue = queue.unwrap_or_else(|| {
            std::env::var("RABBITMQ_QUEUE").unwrap_or_else(|_| "scouter_monitoring".to_string())
        });
        let raise_on_error = raise_on_error.unwrap_or(false);

        RabbitMQConfig {
            address,
            queue,
            raise_on_error,
        }
    }
}
