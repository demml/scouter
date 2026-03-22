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
use tracing::{debug, error, info, info_span, Instrument};

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

    pub fn queue(&self) -> Arc<ArrayQueue<String>> {
        self.queue.clone()
    }

    pub fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    pub fn fingerprint(&self) -> DatasetFingerprint {
        self.fingerprint.clone()
    }

    pub fn namespace(&self) -> DatasetNamespace {
        self.namespace.clone()
    }

    pub fn json_schema(&self) -> String {
        self.json_schema.clone()
    }

    pub fn partition_columns(&self) -> Vec<String> {
        self.partition_columns.clone()
    }

    pub fn grpc_config(&self) -> GrpcConfig {
        self.grpc_config.clone()
    }

    pub fn registered(&self) -> Arc<AtomicBool> {
        self.registered.clone()
    }

    pub fn last_publish(&self) -> Arc<RwLock<DateTime<Utc>>> {
        self.last_publish.clone()
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Insert a JSON string into the queue with backpressure.
    pub async fn insert(&mut self, json_str: String) -> Result<(), EventError> {
        self.insert_with_backpressure(json_str).await?;

        // Check if we need to flush based on capacity
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
    /// Retries up to 3 times with exponential backoff (100ms, 200ms, 400ms).
    async fn insert_with_backpressure(&self, item: String) -> Result<(), EventError> {
        let max_retries = 3;
        let mut current_retry: u32 = 0;

        while current_retry < max_retries {
            match self.queue.push(item.clone()) {
                Ok(_) => return Ok(()),
                Err(_) => {
                    current_retry += 1;
                    if current_retry == max_retries {
                        return Err(EventError::QueuePushError);
                    }
                    // Exponential backoff: 100ms, 200ms, 400ms
                    sleep(Duration::from_millis(100 * 2_u64.pow(current_retry))).await;
                }
            }
        }

        Err(EventError::QueuePushRetryError)
    }

    /// Drain queue, build Arrow batch, send via gRPC.
    pub async fn try_publish(&mut self) -> Result<(), EventError> {
        // Lazy-init gRPC client
        if self.grpc_client.is_none() {
            self.grpc_client =
                Some(DatasetGrpcClient::new(self.grpc_config.clone()).await?);
        }

        let client = self.grpc_client.as_mut().unwrap();

        // Auto-register if not registered
        if !self.registered.load(Ordering::Relaxed) {
            let resp = client
                .register_dataset(
                    &self.namespace.catalog,
                    &self.namespace.schema_name,
                    &self.namespace.table,
                    &self.json_schema,
                    self.partition_columns.clone(),
                )
                .await?;

            // Verify fingerprint matches
            if resp.fingerprint != self.fingerprint.as_str() {
                error!(
                    "Fingerprint mismatch: server={}, local={}",
                    resp.fingerprint,
                    self.fingerprint.as_str()
                );
                return Err(EventError::DatasetFingerprintMismatch);
            }

            self.registered.store(true, Ordering::Relaxed);
            info!("Dataset registered: {}", self.namespace.fqn());
        }

        // Drain queue
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

        // Build Arrow batch
        let mut builder = DynamicBatchBuilder::new(self.schema.clone());
        for json_str in &batch_items {
            builder.append_json_row(json_str).map_err(|e| {
                error!("Failed to append JSON row: {}", e);
                EventError::DatasetBatchBuildError(e.to_string())
            })?;
        }

        let batch = builder.finish().map_err(|e| {
            error!("Failed to finish batch: {}", e);
            EventError::DatasetBatchBuildError(e.to_string())
        })?;

        // Convert to IPC bytes
        let ipc_bytes = batches_to_ipc_bytes(&[batch])?;

        // Send via gRPC
        client
            .insert_batch(
                &self.namespace.catalog,
                &self.namespace.schema_name,
                &self.namespace.table,
                self.fingerprint.as_str(),
                ipc_bytes,
            )
            .await?;

        // Update last publish time
        if let Ok(mut last_publish) = self.last_publish.write() {
            *last_publish = Utc::now();
        }

        info!(
            "Published {} rows to dataset {}",
            batch_items.len(),
            self.namespace.fqn()
        );

        Ok(())
    }

    /// Flush all remaining items in the queue.
    pub async fn flush(&mut self) -> Result<(), EventError> {
        self.try_publish().await
    }
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
/// Mirrors `spawn_queue_event_handler` from `py_queue.rs`.
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
/// Mirrors `BackgroundTask::start_background_task` from `traits/queue.rs`.
#[allow(clippy::too_many_arguments)]
pub fn start_dataset_background_task(
    data_queue: Arc<ArrayQueue<String>>,
    schema: SchemaRef,
    fingerprint: DatasetFingerprint,
    namespace: DatasetNamespace,
    json_schema: String,
    partition_columns: Vec<String>,
    grpc_config: GrpcConfig,
    registered: Arc<AtomicBool>,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    batch_size: usize,
    scheduled_delay_secs: u64,
    task_state: TaskState<DatasetEvent>,
    cancellation_token: CancellationToken,
) -> Result<JoinHandle<()>, EventError> {
    let identifier = namespace.fqn();
    let span = info_span!("dataset_background_task", task = %identifier);

    let future = async move {
        debug!("Starting dataset background task for {}", identifier);

        task_state.set_background_running(true);
        task_state.notify_background_started();
        sleep(Duration::from_millis(10)).await;

        let mut grpc_client: Option<DatasetGrpcClient> = None;

        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(2)) => {
                    debug!("Waking up dataset background task");

                    let now = Utc::now();

                    // Check if enough time has elapsed since last publish
                    let should_process = {
                        if let Ok(last) = last_publish.read() {
                            (now - *last).num_seconds() >= scheduled_delay_secs as i64
                        } else {
                            false
                        }
                    };

                    if should_process {
                        // Drain queue
                        let mut batch_items = Vec::with_capacity(batch_size);
                        while let Some(item) = data_queue.pop() {
                            batch_items.push(item);
                        }

                        // Update last_publish time regardless of batch processing result
                        if let Ok(mut guard) = last_publish.write() {
                            *guard = now;
                        }

                        if !batch_items.is_empty() {
                            // Lazy-init gRPC client
                            if grpc_client.is_none() {
                                match DatasetGrpcClient::new(grpc_config.clone()).await {
                                    Ok(client) => grpc_client = Some(client),
                                    Err(e) => {
                                        error!("Failed to create dataset gRPC client: {}", e);
                                        continue;
                                    }
                                }
                            }

                            let client = grpc_client.as_mut().unwrap();

                            // Auto-register if not registered
                            if !registered.load(Ordering::Relaxed) {
                                match client
                                    .register_dataset(
                                        &namespace.catalog,
                                        &namespace.schema_name,
                                        &namespace.table,
                                        &json_schema,
                                        partition_columns.clone(),
                                    )
                                    .await
                                {
                                    Ok(resp) => {
                                        if resp.fingerprint != fingerprint.as_str() {
                                            error!(
                                                "Fingerprint mismatch: server={}, local={}",
                                                resp.fingerprint,
                                                fingerprint.as_str()
                                            );
                                            continue;
                                        }
                                        registered.store(true, Ordering::Relaxed);
                                        info!("Dataset registered (background): {}", namespace.fqn());
                                    }
                                    Err(e) => {
                                        error!("Failed to register dataset: {}", e);
                                        continue;
                                    }
                                }
                            }

                            // Build Arrow batch
                            let mut builder = DynamicBatchBuilder::new(schema.clone());
                            let mut build_ok = true;
                            for json_str in &batch_items {
                                if let Err(e) = builder.append_json_row(json_str) {
                                    error!("Failed to append JSON row in background task: {}", e);
                                    build_ok = false;
                                    break;
                                }
                            }

                            if build_ok {
                                match builder.finish() {
                                    Ok(batch) => {
                                        match batches_to_ipc_bytes(&[batch]) {
                                            Ok(ipc_bytes) => {
                                                if let Err(e) = client
                                                    .insert_batch(
                                                        &namespace.catalog,
                                                        &namespace.schema_name,
                                                        &namespace.table,
                                                        fingerprint.as_str(),
                                                        ipc_bytes,
                                                    )
                                                    .await
                                                {
                                                    error!("Failed to publish dataset batch: {}", e);
                                                } else {
                                                    info!(
                                                        "Background task published {} rows to dataset {}",
                                                        batch_items.len(),
                                                        namespace.fqn()
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                error!("Failed to convert batch to IPC bytes: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to finish batch in background task: {}", e);
                                    }
                                }
                            }
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
