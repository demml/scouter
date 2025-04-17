// add test logic
use pyo3::prelude::*;
use std::path::PathBuf;
use tracing_subscriber::field::debug;

#[cfg(feature = "server")]
use scouter_server::{start_server_in_background, stop_server};
#[cfg(feature = "server")]
use std::net::TcpListener as StdTcpListener;
#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use std::thread::sleep;
#[cfg(feature = "server")]
use std::time::Duration;
#[cfg(feature = "server")]
use tokio::{runtime::Runtime, sync::Mutex, task::JoinHandle};

#[pyclass]
#[allow(dead_code)]
pub struct ScouterTestServer {
    #[cfg(feature = "server")]
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    #[cfg(feature = "server")]
    runtime: Arc<Runtime>,
    cleanup: bool,
    base_path: Option<PathBuf>,
    rabbit_mq: bool,
    kafka: bool,
}

#[pymethods]
impl ScouterTestServer {
    #[new]
    #[pyo3(signature = (cleanup = true, rabbit_mq = false, kafka = false, base_path = None))]
    fn new(cleanup: bool, rabbit_mq: bool, kafka: bool, base_path: Option<PathBuf>) -> Self {
        ScouterTestServer {
            #[cfg(feature = "server")]
            handle: Arc::new(Mutex::new(None)),
            #[cfg(feature = "server")]
            runtime: Arc::new(Runtime::new().unwrap()),
            cleanup,
            base_path,
            rabbit_mq,
            kafka,
        }
    }

    pub fn set_env_vars_for_client(&self) -> PyResult<()> {
        #[cfg(feature = "server")]
        {
            std::env::set_var("SCOUTER_SERVER_URI", "http://localhost:8000");
            std::env::set_var("APP_ENV", "dev_client");
            Ok(())
        }
        #[cfg(not(feature = "server"))]
        {
            Err(scouter_error::PyScouterError::new_err(
                "Scouter Server feature not enabled",
            ))
        }
    }

    fn start_server(&mut self) -> PyResult<()> {
        #[cfg(feature = "server")]
        {
            println!("Starting Scouter Server...");
            self.cleanup()?;

            // set server env vars
            std::env::set_var("APP_ENV", "dev_server");

            if self.rabbit_mq {
                std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
            }

            if self.kafka {
                std::env::set_var("KAFKA_BROKERS", "localhost:9092");
            }

            let handle = self.handle.clone();
            let runtime = self.runtime.clone();

            let port = match (8000..8010)
                .find(|port| StdTcpListener::bind(("127.0.0.1", *port)).is_ok())
            {
                Some(p) => p,
                None => {
                    return Err(scouter_error::PyScouterError::new_err(
                        "Failed to find available port",
                    ))
                }
            };

            debug!("Found available port: {}", port);

            std::env::set_var("SCOUTER_SERVER_PORT", port.to_string());

            runtime.spawn(async move {
                let server_handle = start_server_in_background();
                *handle.lock().await = server_handle.lock().await.take();
            });

            let client = reqwest::blocking::Client::new();
            let mut attempts = 0;
            let max_attempts = 20;

            while attempts < max_attempts {
                let res = client
                    .get("http://localhost:8000/scouter/healthcheck")
                    .send();

                debug!("Attempt {}: {:?}", attempts, res);

                if let Ok(response) = res {
                    if response.status() == 200 {
                        self.set_env_vars_for_client()?;
                        println!("Scouter Server started successfully");
                        return Ok(());
                    }
                }

                //print response

                attempts += 1;
                sleep(Duration::from_millis(100));
            }

            Err(scouter_error::PyScouterError::new_err(
                "Failed to start Scouter Server",
            ))
        }
        #[cfg(not(feature = "server"))]
        {
            Err(scouter_error::PyScouterError::new_err(
                "Scouter Server feature not enabled",
            ))
        }
    }

    fn stop_server(&self) -> PyResult<()> {
        #[cfg(feature = "server")]
        {
            let handle = self.handle.clone();
            let runtime = self.runtime.clone();
            runtime.spawn(async move {
                stop_server(handle).await;
            });

            if self.cleanup {
                self.cleanup()?;
            }

            Ok(())
        }
        #[cfg(not(feature = "server"))]
        {
            Err(scouter_error::PyScouterError::new_err(
                "Scouter Server feature not enabled",
            ))
        }
    }

    pub fn remove_env_vars_for_client(&self) -> PyResult<()> {
        std::env::remove_var("APP_ENV");
        std::env::remove_var("SCOUTER_SERVER_URI");
        std::env::remove_var("SCOUTER_SERVER_PORT");
        std::env::remove_var("KAFKA_BROKERS");
        std::env::remove_var("RABBITMQ_ADDR");
        Ok(())
    }

    fn cleanup(&self) -> PyResult<()> {
        let current_dir = std::env::current_dir().unwrap();
        let storage_dir = current_dir.join("scouter_storage");

        // unset env vars
        self.remove_env_vars_for_client()?;

        if storage_dir.exists() {
            std::fs::remove_dir_all(storage_dir).unwrap();
        }

        Ok(())
    }

    fn __enter__(mut self_: PyRefMut<Self>) -> PyResult<PyRefMut<Self>> {
        self_.start_server()?;
        Ok(self_)
    }

    fn __exit__(
        &self,
        _exc_type: PyObject,
        _exc_value: PyObject,
        _traceback: PyObject,
    ) -> PyResult<()> {
        self.stop_server()
    }
}

#[pymodule]
pub fn test(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterTestServer>()?;
    Ok(())
}
