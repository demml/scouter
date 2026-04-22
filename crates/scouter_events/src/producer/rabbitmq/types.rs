use pyo3::prelude::*;
use scouter_types::TransportType;

#[pyclass(from_py_object)]
#[derive(Clone, Debug)]
pub struct RabbitMQConfig {
    #[pyo3(get, set)]
    pub address: String,

    #[pyo3(get, set)]
    pub queue: String,

    #[pyo3(get, set)]
    pub max_retries: i32,

    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
impl RabbitMQConfig {
    #[new]
    #[pyo3(signature = (host=None, port=None, username=None, password=None, queue=None, max_retries=3))]
    pub fn new(
        host: Option<String>,
        port: Option<u16>,
        username: Option<String>,
        password: Option<String>,
        queue: Option<String>,
        max_retries: Option<i32>,
    ) -> Self {
        // build address
        let address = std::env::var("RABBITMQ_ADDR").unwrap_or_else(|_| {
            let host = host.unwrap_or_else(|| {
                std::env::var("RABBITMQ_HOST").unwrap_or_else(|_| "localhost".to_string())
            });

            let port = port.unwrap_or_else(|| {
                std::env::var("RABBITMQ_PORT")
                    .unwrap_or_else(|_| "5672".to_string())
                    .parse::<u16>()
                    .unwrap()
            });

            let username = username.or_else(|| std::env::var("RABBITMQ_USERNAME").ok());

            let password = password.or_else(|| std::env::var("RABBITMQ_PASSWORD").ok());

            match (username, password) {
                (Some(user), Some(pass)) => {
                    format!("amqp://{user}:{pass}@{host}:{port}/%2f")
                }
                _ => {
                    format!("amqp://{host}:{port}/%2f")
                }
            }
        });

        let queue = queue.unwrap_or_else(|| {
            std::env::var("RABBITMQ_QUEUE").unwrap_or_else(|_| "scouter_monitoring".to_string())
        });

        RabbitMQConfig {
            address,
            queue,
            max_retries: max_retries.unwrap_or(3),
            transport_type: TransportType::RabbitMQ,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RabbitMQConfig;

    #[test]
    fn rabbitmq_config_uses_env_queue_when_not_provided() {
        unsafe {
            std::env::set_var("RABBITMQ_QUEUE", "isolated_queue");
        }

        let config = RabbitMQConfig::new(None, None, None, None, None, None);

        assert_eq!(config.queue, "isolated_queue");

        unsafe {
            std::env::remove_var("RABBITMQ_QUEUE");
        }
    }

    #[test]
    fn rabbitmq_config_prefers_explicit_queue() {
        unsafe {
            std::env::set_var("RABBITMQ_QUEUE", "isolated_queue");
        }

        let config = RabbitMQConfig::new(
            None,
            None,
            None,
            None,
            Some("explicit_queue".to_string()),
            None,
        );

        assert_eq!(config.queue, "explicit_queue");

        unsafe {
            std::env::remove_var("RABBITMQ_QUEUE");
        }
    }
}
