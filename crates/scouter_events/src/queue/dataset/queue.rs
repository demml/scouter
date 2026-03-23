use crate::error::EventError;
use crate::queue::bus::{Flushable, TaskState};
use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use arrow::ipc::writer::StreamWriter;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_settings::grpc::GrpcConfig;
use scouter_state::app_state;
use scouter_tonic::DatasetGrpcClient;
use scouter_types::dataset::batch_builder::DynamicBatchBuilder;
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, warn, Instrument};

pub enum DatasetEvent {
    Insert(String),
    Flush,
}

impl Flushable for DatasetEvent {
    fn flush_event() -> Self {
        DatasetEvent::Flush
    }
}

pub struct DatasetQueue {
    queue: Arc<ArrayQueue<String>>,
    schema: SchemaRef,
    fingerprint: DatasetFingerprint,
    namespace: DatasetNamespace,
    json_schema: String,
    partition_columns: Vec<String>,
    grpc_config: GrpcConfig,
    registered: Arc<AtomicBool>,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    batch_size: usize,
    grpc_client: Option<DatasetGrpcClient>,
}

impl DatasetQueue {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema: SchemaRef,
        fingerprint: DatasetFingerprint,
        namespace: DatasetNamespace,
        json_schema: String,
        partition_columns: Vec<String>,
        grpc_config: GrpcConfig,
        batch_size: usize,
    ) -> Self {
        let queue = Arc::new(ArrayQueue::new(batch_size * 2));
        Self {
            queue,
            schema,
            fingerprint,
            namespace,
            json_schema,
            partition_columns,
            grpc_config,
            registered: Arc::new(AtomicBool::new(false)),
            last_publish: Arc::new(RwLock::new(Utc::now())),
            batch_size,
            grpc_client: None,
        }
    }

    /// Create a second `DatasetQueue` that shares the same underlying `ArrayQueue`,
    /// `registered` flag, and `last_publish` timestamp. Used so the background flush
    /// task can drain from the same buffer as the event handler.
    #[allow(clippy::too_many_arguments)]
    pub fn with_shared_state(
        queue: Arc<ArrayQueue<String>>,
        schema: SchemaRef,
        fingerprint: DatasetFingerprint,
        namespace: DatasetNamespace,
        json_schema: String,
        partition_columns: Vec<String>,
        grpc_config: GrpcConfig,
        registered: Arc<AtomicBool>,
        last_publish: Arc<RwLock<DateTime<Utc>>>,
        batch_size: usize,
    ) -> Self {
        Self {
            queue,
            schema,
            fingerprint,
            namespace,
            json_schema,
            partition_columns,
            grpc_config,
            registered,
            last_publish,
            batch_size,
            grpc_client: None,
        }
    }

    pub fn queue(&self) -> Arc<ArrayQueue<String>> {
        self.queue.clone()
    }

    pub fn registered(&self) -> Arc<AtomicBool> {
        self.registered.clone()
    }

    pub fn last_publish(&self) -> Arc<RwLock<DateTime<Utc>>> {
        self.last_publish.clone()
    }

    /// Insert a JSON string into the queue with backpressure.
    pub async fn insert(&mut self, json_str: String) -> Result<(), EventError> {
        self.insert_with_backpressure(json_str).await?;

        if self.queue.len() >= self.batch_size {
            debug!(
                "Dataset queue reached capacity, processing queue, current count: {}, batch_size: {}",
                self.queue.len(),
                self.batch_size
            );
            self.try_publish().await?;
        }

        Ok(())
    }

    /// Backpressure handling for inserting items into the queue.
    /// Retries up to 3 times with exponential backoff (200ms, 400ms, 800ms).
    /// `ArrayQueue::push` returns the item on failure, so no cloning is needed.
    async fn insert_with_backpressure(&self, item: String) -> Result<(), EventError> {
        match self.queue.push(item) {
            Ok(_) => Ok(()),
            Err(returned) => {
                let mut item = returned;
                for retry in 1..=3u32 {
                    sleep(Duration::from_millis(100 * 2_u64.pow(retry))).await;
                    match self.queue.push(item) {
                        Ok(_) => return Ok(()),
                        Err(returned) => {
                            item = returned;
                        }
                    }
                }
                Err(EventError::QueuePushError)
            }
        }
    }

    /// Drain queue, build Arrow batch, send via gRPC.
    /// On failure, items are re-queued on a best-effort basis.
    pub async fn try_publish(&mut self) -> Result<(), EventError> {
        // Lazy-init gRPC client
        if self.grpc_client.is_none() {
            self.grpc_client = Some(DatasetGrpcClient::new(self.grpc_config.clone()).await?);
        }

        // Auto-register if not registered
        if !self.registered.load(Ordering::Acquire) {
            let client = self.grpc_client.as_mut().unwrap();
            let resp = client
                .register_dataset(
                    &self.namespace.catalog,
                    &self.namespace.schema_name,
                    &self.namespace.table,
                    &self.json_schema,
                    self.partition_columns.clone(),
                )
                .await?;

            if resp.fingerprint != self.fingerprint.as_str() {
                error!(
                    "Fingerprint mismatch: server={}, local={}",
                    resp.fingerprint,
                    self.fingerprint.as_str()
                );
                return Err(EventError::DatasetFingerprintMismatch);
            }

            self.registered.store(true, Ordering::Release);
            info!("Dataset registered: {}", self.namespace.fqn());
        }

        // Drain at most batch_size items
        let mut batch_items = Vec::with_capacity(self.batch_size);
        while let Some(item) = self.queue.pop() {
            batch_items.push(item);
            if batch_items.len() >= self.batch_size {
                break;
            }
        }

        if batch_items.is_empty() {
            return Ok(());
        }

        // Build Arrow batch from JSON strings
        let row_count = batch_items.len();
        let send_result = build_and_send(
            self.grpc_client.as_mut().unwrap(),
            &self.schema,
            &self.namespace,
            &self.fingerprint,
            &batch_items,
        )
        .await;

        match send_result {
            Ok(()) => {
                if let Ok(mut last_publish) = self.last_publish.write() {
                    *last_publish = Utc::now();
                }
                info!(
                    "Published {} rows to dataset {}",
                    row_count,
                    self.namespace.fqn()
                );
                Ok(())
            }
            Err(e) => {
                // Re-queue items on best-effort basis
                let mut dropped = 0;
                for item in batch_items {
                    if self.queue.push(item).is_err() {
                        dropped += 1;
                    }
                }
                if dropped > 0 {
                    warn!(
                        "Dropped {} rows (queue full after failed publish to {})",
                        dropped,
                        self.namespace.fqn()
                    );
                }
                Err(e)
            }
        }
    }

    /// Flush all remaining items in the queue.
    pub async fn flush(&mut self) -> Result<(), EventError> {
        self.try_publish().await
    }

    /// Check whether enough time has elapsed since the last publish.
    pub fn should_process(&self, scheduled_delay_secs: u64) -> bool {
        if let Ok(last) = self.last_publish.read() {
            (Utc::now() - *last).num_seconds() >= scheduled_delay_secs as i64
        } else {
            false
        }
    }
}

/// Build an Arrow batch from JSON strings and send via gRPC.
async fn build_and_send(
    client: &mut DatasetGrpcClient,
    schema: &SchemaRef,
    namespace: &DatasetNamespace,
    fingerprint: &DatasetFingerprint,
    batch_items: &[String],
) -> Result<(), EventError> {
    let mut builder = DynamicBatchBuilder::new(schema.clone());
    for json_str in batch_items {
        builder.append_json_row(json_str).map_err(|e| {
            error!("Failed to append JSON row: {}", e);
            EventError::DatasetBatchBuildError(e.to_string())
        })?;
    }

    let batch = builder.finish().map_err(|e| {
        error!("Failed to finish batch: {}", e);
        EventError::DatasetBatchBuildError(e.to_string())
    })?;

    let ipc_bytes = batches_to_ipc_bytes(&[batch])?;

    client
        .insert_batch(
            &namespace.catalog,
            &namespace.schema_name,
            &namespace.table,
            fingerprint.as_str(),
            ipc_bytes,
        )
        .await?;

    Ok(())
}

/// Convert Arrow RecordBatches to IPC stream bytes.
fn batches_to_ipc_bytes(batches: &[RecordBatch]) -> Result<Vec<u8>, EventError> {
    if batches.is_empty() {
        return Ok(Vec::new());
    }

    let schema = batches[0].schema();
    let mut buf = Vec::new();

    {
        let mut writer = StreamWriter::try_new(&mut buf, &schema).map_err(|e| {
            EventError::DatasetBatchBuildError(format!("Failed to create IPC writer: {e}"))
        })?;

        for batch in batches {
            writer.write(batch).map_err(|e| {
                EventError::DatasetBatchBuildError(format!("Failed to write IPC batch: {e}"))
            })?;
        }

        writer.finish().map_err(|e| {
            EventError::DatasetBatchBuildError(format!("Failed to finish IPC stream: {e}"))
        })?;
    }

    Ok(buf)
}

/// Event handler loop for the dataset queue.
pub async fn spawn_dataset_event_handler(
    mut event_rx: UnboundedReceiver<DatasetEvent>,
    mut queue: DatasetQueue,
    task_state: TaskState<DatasetEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), EventError> {
    task_state.set_event_running(true);
    task_state.notify_event_started();
    debug!("Dataset event handler started");

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
                match event {
                    DatasetEvent::Insert(json) => {
                        if let Err(e) = queue.insert(json).await {
                            error!("Error inserting into dataset queue: {}", e);
                        }
                    }
                    DatasetEvent::Flush => {
                        if let Err(e) = queue.flush().await {
                            error!("Error flushing dataset queue: {}", e);
                        }
                    }
                }
            }

            _ = cancellation_token.cancelled() => {
                debug!("Stop signal received for dataset event handler");
                if let Err(e) = queue.flush().await {
                    error!("Error flushing dataset queue during shutdown: {}", e);
                }
                task_state.set_event_running(false);
                break;
            }

            else => {
                debug!("Dataset event channel closed, shutting down");
                if let Err(e) = queue.flush().await {
                    error!("Error flushing dataset queue during channel close: {}", e);
                }
                task_state.set_event_running(false);
                break;
            }
        }
    }

    Ok(())
}

/// Background flush task for the dataset queue.
/// Periodically drains the queue and publishes via gRPC using `DatasetQueue::try_publish`.
pub fn start_dataset_background_task(
    mut queue: DatasetQueue,
    scheduled_delay_secs: u64,
    task_state: TaskState<DatasetEvent>,
    cancellation_token: CancellationToken,
) -> Result<JoinHandle<()>, EventError> {
    let identifier = queue.namespace.fqn();
    let span = info_span!("dataset_background_task", task = %identifier);

    let future = async move {
        debug!("Starting dataset background task for {}", identifier);

        task_state.set_background_running(true);
        task_state.notify_background_started();
        sleep(Duration::from_millis(10)).await;

        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(2)) => {
                    if queue.should_process(scheduled_delay_secs) {
                        if let Err(e) = queue.try_publish().await {
                            error!("Background publish failed for {}: {}", identifier, e);
                        }
                    }
                }

                _ = cancellation_token.cancelled() => {
                    info!("Stop signal received, shutting down dataset background task");
                    task_state.set_background_running(false);
                    break;
                }

                else => {
                    info!("Dataset background task channel closed, shutting down");
                    task_state.set_background_running(false);
                    break;
                }
            }
        }

        debug!("Dataset background task finished");
    };

    let handle = app_state()
        .handle()
        .spawn(async move { future.instrument(span).await });

    Ok(handle)
}
