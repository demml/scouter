use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::RwLock;

use pyo3::prelude::*;
use scouter_events::queue::bus::{Task, TaskState};
use scouter_events::queue::dataset::{
    spawn_dataset_event_handler, start_dataset_background_task, DatasetEvent, DatasetQueue,
};
use scouter_events::queue::traits::queue::{wait_for_background_task, wait_for_event_task};
use scouter_settings::grpc::GrpcConfig;
use scouter_state::app_state;
use scouter_tonic::DatasetGrpcClient;
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use super::config::{TableConfig, WriteConfig};
use super::error::DatasetClientError;

#[pyclass]
pub struct DatasetProducer {
    task_state: Option<TaskState<DatasetEvent>>,
    namespace: DatasetNamespace,
    fingerprint: DatasetFingerprint,
    json_schema: String,
    partition_columns: Vec<String>,
    grpc_config: GrpcConfig,
    registered: Arc<AtomicBool>,
}

#[pymethods]
impl DatasetProducer {
    #[new]
    #[pyo3(signature = (table_config, transport, write_config=None))]
    fn new(
        table_config: &TableConfig,
        transport: &Bound<'_, PyAny>,
        write_config: Option<&WriteConfig>,
    ) -> Result<Self, DatasetClientError> {
        let grpc_config = transport.extract::<GrpcConfig>().map_err(|_| {
            DatasetClientError::PyError("transport must be a GrpcConfig instance".to_string())
        })?;
        let wc = write_config.cloned().unwrap_or_default();
        let registered = Arc::new(AtomicBool::new(false));

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<DatasetEvent>();

        let mut task_state = TaskState {
            event_task: Arc::new(RwLock::new(Task::new())),
            background_task: Arc::new(RwLock::new(Task::new())),
            event_tx,
            id: table_config.namespace.fqn(),
        };

        let dataset_queue = DatasetQueue::new(
            table_config.schema.clone(),
            table_config.fingerprint.clone(),
            table_config.namespace.clone(),
            table_config.json_schema.clone(),
            table_config.partition_columns.clone(),
            grpc_config.clone(),
            wc.batch_size,
        );

        let shared_queue = dataset_queue.queue();
        let shared_registered = dataset_queue.registered();
        let shared_last_publish = dataset_queue.last_publish();

        // Spawn event handler
        let event_cancel = CancellationToken::new();
        let ec = event_cancel.clone();
        let ts_clone = task_state.clone();
        let event_handle = app_state().handle().spawn(async move {
            if let Err(e) =
                spawn_dataset_event_handler(event_rx, dataset_queue, ts_clone, ec).await
            {
                tracing::error!("Dataset event handler error: {e}");
            }
        });
        task_state.add_event_abort_handle(event_handle);
        task_state.add_event_cancellation_token(event_cancel);

        // Spawn background flush task
        let bg_cancel = CancellationToken::new();
        let bg_handle = start_dataset_background_task(
            shared_queue,
            table_config.schema.clone(),
            table_config.fingerprint.clone(),
            table_config.namespace.clone(),
            table_config.json_schema.clone(),
            table_config.partition_columns.clone(),
            grpc_config.clone(),
            shared_registered,
            shared_last_publish,
            wc.batch_size,
            wc.scheduled_delay_secs,
            task_state.clone(),
            bg_cancel.clone(),
        )?;
        task_state.add_background_abort_handle(bg_handle);
        task_state.add_background_cancellation_token(bg_cancel);

        // Wait for both tasks to start
        app_state().handle().block_on(async {
            wait_for_background_task(&task_state).await?;
            wait_for_event_task(&task_state).await
        })?;

        debug!(
            "DatasetProducer initialized for '{}'",
            table_config.namespace.fqn()
        );

        Ok(Self {
            task_state: Some(task_state),
            namespace: table_config.namespace.clone(),
            fingerprint: table_config.fingerprint.clone(),
            json_schema: table_config.json_schema.clone(),
            partition_columns: table_config.partition_columns.clone(),
            grpc_config,
            registered,
        })
    }

    fn insert(&self, record: &Bound<'_, PyAny>) -> Result<(), DatasetClientError> {
        let ts = self
            .task_state
            .as_ref()
            .ok_or(DatasetClientError::AlreadyShutdown)?;
        let json = record.call_method0("model_dump_json")?.extract::<String>()?;
        ts.event_tx.send(DatasetEvent::Insert(json))?;
        Ok(())
    }

    fn flush(&self) -> Result<(), DatasetClientError> {
        let ts = self
            .task_state
            .as_ref()
            .ok_or(DatasetClientError::AlreadyShutdown)?;
        ts.event_tx.send(DatasetEvent::Flush)?;
        Ok(())
    }

    fn shutdown(&mut self, py: Python<'_>) -> Result<(), DatasetClientError> {
        if let Some(task_state) = self.task_state.take() {
            py.detach(|| task_state.shutdown_tasks())?;
        }
        Ok(())
    }

    fn register(&self) -> Result<String, DatasetClientError> {
        let grpc_config = self.grpc_config.clone();
        let catalog = self.namespace.catalog.clone();
        let schema_name = self.namespace.schema_name.clone();
        let table = self.namespace.table.clone();
        let json_schema = self.json_schema.clone();
        let partition_columns = self.partition_columns.clone();
        let registered = self.registered.clone();

        app_state().block_on(async move {
            let mut client = DatasetGrpcClient::new(grpc_config).await?;
            let resp = client
                .register_dataset(&catalog, &schema_name, &table, &json_schema, partition_columns)
                .await?;
            registered.store(true, Ordering::Release);
            info!(
                "DatasetProducer: registered '{catalog}.{schema_name}.{table}' — {}",
                resp.status
            );
            Ok(resp.status)
        })
    }

    #[getter]
    fn fingerprint(&self) -> String {
        self.fingerprint.as_str().to_string()
    }

    #[getter]
    fn namespace(&self) -> String {
        self.namespace.fqn()
    }

    #[getter]
    fn is_registered(&self) -> bool {
        self.registered.load(Ordering::Acquire)
    }
}
