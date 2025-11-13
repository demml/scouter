// implements a BackgroundQueue trait

use crate::error::{EventError, FeatureQueueError};
use crate::producer::RustScouterProducer;
use crate::queue::bus::TaskState;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::MessageRecord;
use scouter_types::QueueExt;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, Instrument};

pub trait FeatureQueue: Send + Sync {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<MessageRecord, FeatureQueueError>;
}

pub trait BackgroundTask: Send + Sync + 'static {
    type DataItem: QueueExt + Send + Sync + 'static;
    type Processor: FeatureQueue + Send + Sync + 'static;

    #[allow(clippy::too_many_arguments)]
    fn start_background_task(
        &self,
        data_queue: Arc<ArrayQueue<Self::DataItem>>,
        processor: Arc<Self::Processor>,
        mut producer: RustScouterProducer,
        last_publish: Arc<RwLock<DateTime<Utc>>>,
        runtime: Arc<Runtime>,
        queue_capacity: usize,
        identifier: String,
        task_state: TaskState,
        cancellation_token: CancellationToken,
    ) -> Result<JoinHandle<()>, EventError> {
        let span = info_span!("background_task", task = %identifier);

        let future = async move {
            debug!("Starting background task");

            // Set running state immediately
            task_state.set_background_running(true);
            debug!("Background task set to running");

            // Small delay to ensure state is propagated
            sleep(Duration::from_millis(10)).await;
            loop {
                tokio::select! {
                    _ = sleep(Duration::from_secs(2)) => {
                        debug!("Waking up background task");

                        let now = Utc::now();

                        // Scope the read guard to drop it before the future is sent
                        let should_process = {
                            if let Ok(last) = last_publish.read() {
                                (now - *last).num_seconds() >= 30
                            } else {
                                false
                            }
                        };

                        if should_process {
                            let mut batch = Vec::with_capacity(queue_capacity);
                            while let Some(item) = data_queue.pop() {
                                batch.push(item);
                            }

                            // Always update last_publish time, regardless of batch processing result
                            if let Ok(mut guard) = last_publish.write() {
                                *guard = now;
                            }

                            if !batch.is_empty() {
                                match processor.create_drift_records_from_batch(batch) {
                                    Ok(records) => {

                                        // publish
                                        if let Err(e) = producer.publish(records).await {
                                            error!("Failed to publish records: {}", e);
                                        } else {
                                            info!("Successfully published records");
                                        }
                                    }
                                    Err(e) => error!("Failed to create drift records: {}", e),
                                }
                            }

                        }
                    }
                    _ = cancellation_token.cancelled()  => {
                        info!("Stop signal received, shutting down background task");
                        task_state.set_background_running(false);
                        break;
                    }
                    else =>  {
                        info!("Stop signal received, shutting down background task");
                        task_state.set_background_running(false);
                        break;
                    }
                }
            }
            debug!("Background task finished");
        };

        let handle = runtime.spawn(async move { future.instrument(span).await });
        Ok(handle)
    }
}

/// This is a primary trait implemented on all queues
/// It provides the basic functionality for inserting, publishing, and flushing
#[async_trait]
pub trait QueueMethods {
    type ItemType: QueueExt + 'static + Clone + Debug;
    type FeatureQueue: FeatureQueue + 'static;

    /// These all need to be implemented in the concrete queue type
    fn capacity(&self) -> usize;
    fn get_producer(&mut self) -> &mut RustScouterProducer;
    fn queue(&self) -> Arc<ArrayQueue<Self::ItemType>>;
    fn feature_queue(&self) -> Arc<Self::FeatureQueue>;
    fn last_publish(&self) -> Arc<RwLock<DateTime<Utc>>>;
    fn should_process(&self, current_count: usize) -> bool;

    fn update_last_publish(&mut self) -> Result<(), EventError> {
        if let Ok(mut last_publish) = self.last_publish().write() {
            *last_publish = Utc::now();
        }

        Ok(())
    }

    /// Publish the records to the producer
    /// Remember - everything flows down from python, so the async producers need
    /// to be called in a blocking manner
    async fn publish(&mut self, records: MessageRecord) -> Result<(), EventError> {
        let producer = self.get_producer();
        producer.publish(records).await
    }

    /// Insert an item into the queue
    async fn insert(&mut self, item: Self::ItemType) -> Result<(), EventError> {
        debug!("Inserting item into queue: {:?}", item);

        self.insert_with_backpressure(item).await?;

        let queue = self.queue();

        // Check if we need to process the queue
        // queues have a buffer in case of overflow, so we need to check if we are over the capacity, which is smaller
        if queue.len() >= self.capacity() {
            debug!(
                "Queue reached capacity, processing queue, current count: {}, current_capacity: {}",
                queue.len(),
                self.capacity()
            );
            self.try_publish(queue.clone()).await?;
        }

        Ok(())
    }

    /// Process the queue and publish records
    async fn try_publish(
        &mut self,
        queue: Arc<ArrayQueue<Self::ItemType>>,
    ) -> Result<(), EventError> {
        let mut batch = Vec::with_capacity(queue.capacity());

        while let Some(metrics) = queue.pop() {
            batch.push(metrics);
        }

        if !batch.is_empty() {
            let feature_queue = self.feature_queue();
            match feature_queue.create_drift_records_from_batch(batch) {
                Ok(records) => {
                    self.publish(records).await?;
                    self.update_last_publish()?;
                }
                Err(e) => error!("Failed to create drift records: {}", e),
            }
        }

        Ok(())
    }

    /// Flush the queue and shut down background tasks
    async fn flush(&mut self) -> Result<(), EventError>;

    /// Backpressure handling for inserting items into the queue
    /// This will retry inserting the item a few times with exponential backoff
    async fn insert_with_backpressure(&mut self, item: Self::ItemType) -> Result<(), EventError> {
        let queue = self.queue();
        let max_retries = 3;
        let mut current_retry: u32 = 0;

        while current_retry < max_retries {
            match queue.push(item.clone()) {
                Ok(_) => return Ok(()),
                Err(_) => {
                    current_retry += 1;
                    if current_retry == max_retries {
                        return Err(EventError::QueuePushError);
                    }
                    // Added exponential backoff: 100ms, 200ms, 400ms
                    sleep(Duration::from_millis(100 * 2_u64.pow(current_retry))).await;
                }
            }
        }

        Err(EventError::QueuePushRetryError)
    }
}

/// Waits for the background loop to start
pub fn wait_for_background_task(task_state: &TaskState) -> Result<(), EventError> {
    // Signal confirm start
    if task_state.has_background_handle() {
        let mut max_retries = 50;
        while max_retries > 0 {
            if task_state.is_background_running() {
                debug!("Background loop started successfully");
                return Ok(());
            }
            max_retries -= 1;
            std::thread::sleep(Duration::from_millis(200));
        }
        error!("Background task failed to start");
        Err(EventError::BackgroundTaskFailedToStartError)
    } else {
        debug!("No background handle to wait for");
        Ok(())
    }
}

/// Waits for the event task to start
pub fn wait_for_event_task(task_state: &TaskState) -> Result<(), EventError> {
    // Signal confirm start

    let mut max_retries = 50;
    while max_retries > 0 {
        if task_state.is_event_running() {
            debug!("Event task started successfully");
            return Ok(());
        }
        max_retries -= 1;
        std::thread::sleep(Duration::from_millis(200));
    }
    error!("Event task failed to start");
    Err(EventError::EventTaskFailedToStartError)
}
