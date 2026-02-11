// add test logic
use potato_head::mock::LLMTestServer;
use pyo3::prelude::*;
use std::path::PathBuf;
use thiserror::Error;
use tracing::instrument;

#[cfg(feature = "server")]
use scouter_server::{start_server_in_background, stop_server};
#[cfg(feature = "server")]
use scouter_tonic::GrpcClient;
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
#[cfg(feature = "server")]
use tracing::debug;

#[derive(Error, Debug)]
pub enum TestServerError {
    #[error("Failed to find port")]
    PortError,

    #[error("Failed to start server")]
    StartServerError,

    #[error("Server feature not enabled")]
    FeatureNotEnabled,

    #[error("{0}")]
    RuntimeError(String),
}

impl From<TestServerError> for PyErr {
    fn from(err: TestServerError) -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(err.to_string())
    }
}

impl From<PyErr> for TestServerError {
    fn from(err: PyErr) -> TestServerError {
        TestServerError::RuntimeError(err.to_string())
    }
}

#[pyclass]
#[allow(dead_code)]
pub struct ScouterTestServer {
    #[cfg(feature = "server")]
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    #[cfg(feature = "server")]
    runtime: Arc<Runtime>,
    openai_server: Option<LLMTestServer>,
    cleanup: bool,
    base_path: Option<PathBuf>,
    rabbit_mq: bool,
    kafka: bool,
    openai: bool,
}

#[pymethods]
impl ScouterTestServer {
    #[new]
    #[pyo3(signature = (cleanup = true, rabbit_mq = false, kafka = false, openai = false, base_path = None))]
    fn new(
        cleanup: bool,
        rabbit_mq: bool,
        kafka: bool,
        openai: bool,
        base_path: Option<PathBuf>,
    ) -> Self {
        ScouterTestServer {
            #[cfg(feature = "server")]
            handle: Arc::new(Mutex::new(None)),
            #[cfg(feature = "server")]
            runtime: Arc::new(Runtime::new().unwrap()),
            openai_server: None,
            cleanup,
            base_path,
            rabbit_mq,
            kafka,
            openai,
        }
    }

    #[cfg(feature = "server")]
    pub fn set_env_vars_for_client(
        &self,
        http_port: u16,
        grpc_port: u16,
    ) -> Result<(), TestServerError> {
        unsafe {
            std::env::set_var(
                "SCOUTER_SERVER_URI",
                format!("http://localhost:{http_port}"),
            );
            std::env::set_var("SCOUTER_GRPC_URI", format!("http://localhost:{grpc_port}"));
            std::env::set_var("APP_ENV", "dev_client");
        }
        Ok(())
    }

    #[instrument(name = "start_mock_server", skip_all)]
    fn start_server(&mut self) -> Result<(), TestServerError> {
        #[cfg(feature = "server")]
        {
            println!("Starting Scouter Server...");
            self.cleanup()?;

            // set server env vars
            unsafe {
                std::env::set_var("APP_ENV", "dev_server");
            }

            if self.rabbit_mq {
                unsafe {
                    std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
                }
            }

            if self.kafka {
                unsafe {
                    std::env::set_var("KAFKA_BROKERS", "localhost:9092");
                }
            }

            if self.openai {
                let mut server = LLMTestServer::new();
                server.start_server().unwrap();
                self.openai_server = Some(server);

                println!("Started OpenAI Test Server");
                // print env vars for OpenAI
                println!(
                    "OpenAI API Key: {}",
                    std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "Not set".to_string())
                );
                println!(
                    "OpenAI API URL: {}",
                    std::env::var("OPENAI_API_URL").unwrap_or_else(|_| "Not set".to_string())
                );
            }

            let handle = self.handle.clone();
            let runtime = self.runtime.clone();

            let http_port = (3000..3010)
                .find(|port| StdTcpListener::bind(("127.0.0.1", *port)).is_ok())
                .ok_or(TestServerError::PortError)?;

            let grpc_port = (50051..50061)
                .find(|port| StdTcpListener::bind(("127.0.0.1", *port)).is_ok())
                .ok_or(TestServerError::PortError)?;

            debug!("Found ports - HTTP: {}, gRPC: {}", http_port, grpc_port);

            unsafe {
                std::env::set_var("SCOUTER_SERVER_PORT", http_port.to_string());
                std::env::set_var("SCOUTER_GRPC_PORT", grpc_port.to_string());
            }

            runtime.spawn(async move {
                let server_handle = start_server_in_background();
                *handle.lock().await = server_handle.lock().await.take();
            });

            let client = reqwest::blocking::Client::new();
            let runtime_clone = self.runtime.clone();
            let mut attempts = 0;
            let max_attempts = 50; // Increased timeout

            while attempts < max_attempts {
                println!(
                    "🔍 Checking servers (attempt {}/{}): HTTP:{}, gRPC:{}",
                    attempts + 1,
                    max_attempts,
                    http_port,
                    grpc_port
                );

                // Check HTTP health
                let http_ready = client
                    .get(format!("http://localhost:{http_port}/scouter/healthcheck"))
                    .timeout(Duration::from_secs(2))
                    .send()
                    .map(|r| r.status().is_success())
                    .unwrap_or(false);

                // Check gRPC health using standard health service
                let grpc_ready = runtime_clone.block_on(async {
                    let config = scouter_client::GrpcConfig::new(
                        Some(format!("http://127.0.0.1:{grpc_port}")),
                        Some("guest".to_string()),
                        Some("guest".to_string()),
                    );

                    // Use the health check method
                    match GrpcClient::new(config).await {
                        Ok(client) => client.health_check().await.unwrap_or(false),
                        Err(_) => false,
                    }
                });

                if http_ready && grpc_ready {
                    self.set_env_vars_for_client(http_port, grpc_port)?;
                    println!(
                        "✅ Both servers ready! HTTP:{}, gRPC:{}",
                        http_port, grpc_port
                    );
                    return Ok(());
                }

                println!(
                    "  ⏳ HTTP: {}, gRPC: {}",
                    if http_ready { "✓" } else { "✗" },
                    if grpc_ready { "✓" } else { "✗" }
                );

                attempts += 1;
                sleep(Duration::from_millis(100 + (attempts * 20)));
            }

            Err(TestServerError::StartServerError)
        }
        #[cfg(not(feature = "server"))]
        {
            Err(TestServerError::FeatureNotEnabled)
        }
    }

    fn stop_server(&mut self) -> Result<(), TestServerError> {
        #[cfg(feature = "server")]
        {
            let handle = self.handle.clone();
            let runtime = self.runtime.clone();
            runtime.spawn(async move {
                stop_server(handle).await;
            });

            if self.openai {
                debug!("Stopping OpenAI Test Server...");
                if let Some(server) = &mut self.openai_server {
                    server.stop_server().unwrap();
                }
                debug!("OpenAI Test Server stopped");
            }

            if self.cleanup {
                self.cleanup()?;
            }

            Ok(())
        }
        #[cfg(not(feature = "server"))]
        {
            Err(TestServerError::FeatureNotEnabled)
        }
    }

    pub fn remove_env_vars_for_client(&self) -> PyResult<()> {
        unsafe {
            std::env::remove_var("APP_ENV");
            std::env::remove_var("SCOUTER_SERVER_URI");
            std::env::remove_var("SCOUTER_SERVER_PORT");
            std::env::remove_var("KAFKA_BROKERS");
            std::env::remove_var("RABBITMQ_ADDR");
        }
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

    fn __enter__(mut self_: PyRefMut<Self>) -> Result<PyRefMut<Self>, TestServerError> {
        self_.start_server()?;
        Ok(self_)
    }

    fn __exit__(
        &mut self,
        _exc_type: Py<PyAny>,
        _exc_value: Py<PyAny>,
        _traceback: Py<PyAny>,
    ) -> Result<(), TestServerError> {
        self.stop_server()
    }
}
