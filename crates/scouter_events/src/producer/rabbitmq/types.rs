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
    #[pyo3(signature = (host=None, port=None, username=None, password=None, queue="scouter_monitoring".to_string(), raise_on_error=false))]
    pub fn new(
        host: Option<String>,
        port: Option<u16>,
        username: Option<String>,
        password: Option<String>,
        queue: Option<String>,
        raise_on_error: Option<bool>,
    ) -> Self {

        // build address
        let address =  std::env::var("RABBITMQ_ADDRESS")
            .unwrap_or_else(|_| {
               
                    let host = host.unwrap_or_else(|| {
                        std::env::var("RABBITMQ_HOST").unwrap_or_else(|_| "localhost".to_string())
                    });

                    let port = port.unwrap_or_else(|| {
                        std::env::var("RABBITMQ_PORT").unwrap_or_else(|_| "5672".to_string())
                            .parse::<u16>()
                            .unwrap()
                    });

                    let username = username.or_else(|| {
                        std::env::var("RABBITMQ_USERNAME").ok()
                    });

                    let password = password.or_else(|| {
                        std::env::var("RABBITMQ_PASSWORD").ok()
                    });
                    
                    if username.is_none() || password.is_none() {
                        return format!("amqp://{}:{}/%2f", host, port)
                    } else {
                        return format!("amqp://{}:{}@{}:{}/%2f", username.unwrap(), password.unwrap(), host, port)
                    }
            
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
