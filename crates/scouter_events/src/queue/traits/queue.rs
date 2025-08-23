// implements a BackgroundQueue trait

use crate::error::{EventError, FeatureQueueError};
use crate::producer::RustScouterProducer;
use crate::queue::bus::EventLoops;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::QueueExt;
use scouter_types::ServerRecords;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

use tokio::runtime::Runtime;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, info_span, Instrument};

pub trait FeatureQueue: Send + Sync {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<ServerRecords, FeatureQueueError>;
}

pub enum BackgroundEvent {
    Start,
    Stop,
}

async fn process_batch<D, P>(
    queue_capacity: Option<usize>,
    data_queue: Arc<ArrayQueue<D>>,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    processor: Arc<P>,
    producer: Arc<Mutex<RustScouterProducer>>,
    now: DateTime<Utc>,
) where
    D: QueueExt + Send + Sync + 'static,
    P: FeatureQueue + Send + Sync + 'static,
{
    let mut batch = if let Some(capacity) = queue_capacity {
        Vec::with_capacity(capacity)
    } else {
        Vec::new()
    };
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
                // acquire lock producer mutex
                let mut producer = producer.lock().await;

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

pub trait BackgroundTask: Send + Sync + 'static {
    type DataItem: QueueExt + Send + Sync + 'static;
    type Processor: FeatureQueue + Send + Sync + 'static;

    #[allow(clippy::too_many_arguments)]
    fn start_background_task(
        &self,
        data_queue: Arc<ArrayQueue<Self::DataItem>>,
        processor: Arc<Self::Processor>,
        producer: Arc<Mutex<RustScouterProducer>>,
        last_publish: Arc<RwLock<DateTime<Utc>>>,
        runtime: Arc<Runtime>,
        mut stop_rx: watch::Receiver<()>,
        queue_capacity: usize,
        label: &'static str,
        event_loops: EventLoops,
        mut background_rx: UnboundedReceiver<BackgroundEvent>,
    ) -> Result<JoinHandle<()>, EventError> {
        let future = async move {
            debug!("Starting background task: {}", label);

            // Set running state immediately
            event_loops.set_background_loop_running(true);
            debug!("Background task {} set to running", label);

            // Small delay to ensure state is propagated
            sleep(Duration::from_millis(10)).await;
            loop {
                tokio::select! {
                    Some(event) = background_rx.recv() => {
                        match event {
                            BackgroundEvent::Start => {
                                debug!("Background task {} received start event", label);
                            }
                            BackgroundEvent::Stop => {
                                debug!("Background task {} received stop event", label);
                                break;
                            }
                        }
                    }
                    _ = sleep(Duration::from_secs(1)) => {
                        debug!("Waking up background task: {}", label);
                        if !event_loops.is_background_loop_running() {
                            continue;
                        }

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
                            debug!("Processing queued data");
                            process_batch(
                                Some(queue_capacity),
                                data_queue.clone(),
                                last_publish.clone(),
                                processor.clone(),
                                producer.clone(),
                                now,
                            ).await;

                        }
                    },
                    _ = stop_rx.changed() => {
                        info!("Stop signal received, shutting down background task: {}", label);
                        // Stop the background task and publish remaining records
                        process_batch(
                                None,
                                data_queue.clone(),
                                last_publish.clone(),
                                processor.clone(),
                                producer.clone(),
                                Utc::now(),
                            ).await;
                        event_loops.set_background_loop_running(false);
                        break;
                    }
                }
            }
            debug!("Background task finished");
        };

        let span = info_span!("background_task", task = %label);
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
    async fn publish(&mut self, records: ServerRecords) -> Result<(), EventError> {
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
            self.try_publish(queue).await?;
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
        let mut current_retry = 0;

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
pub fn wait_for_background_task(event_loops: &EventLoops) -> Result<(), EventError> {
    // Signal confirm start
    if event_loops.has_background_handle() {
        let mut max_retries = 20;
        while max_retries > 0 {
            if event_loops.is_background_loop_running() {
                debug!("Background loop started successfully");
                return Ok(());
            }
            max_retries -= 1;
            std::thread::sleep(Duration::from_millis(200));
        }
        error!("Background loop failed to start");
        Err(EventError::BackgroundLoopFailedToStartError)
    } else {
        debug!("No background handle to wait for");
        Ok(())
    }
}

/// Waits for the event task to start
pub fn wait_for_event_task(event_loops: &EventLoops) -> Result<(), EventError> {
    // Signal confirm start

    let mut max_retries = 20;
    while max_retries > 0 {
        event_loops.start_event_task()?;
        if event_loops.is_event_loop_running() {
            debug!("Event loop started successfully");
            return Ok(());
        }
        max_retries -= 1;
        std::thread::sleep(Duration::from_millis(200));
    }
    error!("Event loop failed to start");
    Err(EventError::EventLoopFailedToStartError)
}
