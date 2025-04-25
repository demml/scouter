// implements a BackgroundQueue trait

use crate::producer::RustScouterProducer;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_error::EventError;
use scouter_error::FeatureQueueError;
use scouter_types::QueueExt;
use scouter_types::ServerRecords;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime::Runtime;
use tokio::sync::watch;
use tokio::time::{self, Duration};
use tracing::{debug, error, info, info_span, Instrument};
pub trait FeatureQueue: Send + Sync {
    fn create_drift_records_from_batch<T: QueueExt>(
        &self,
        batch: Vec<T>,
    ) -> Result<ServerRecords, FeatureQueueError>;
}

pub trait BackgroundTask {
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
        mut stop_rx: watch::Receiver<()>,
        queue_capacity: usize,
        label: &'static str,
    ) -> Result<(), EventError> {
        let future = async move {
            loop {
                tokio::select! {
                    _ = time::sleep(Duration::from_secs(2)) => {
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

                            let mut batch = Vec::with_capacity(queue_capacity);
                            while let Some(item) = data_queue.pop() {
                                batch.push(item);
                            }

                            if !batch.is_empty() {
                                match processor.create_drift_records_from_batch(batch) {
                                    Ok(records) => {
                                        if let Err(e) = producer.publish(records).await {
                                            error!("Failed to publish records: {}", e);
                                        } else {
                                            // Scope the write guard to drop it
                                            {
                                                if let Ok(mut guard) = last_publish.write() {
                                                    *guard = now;
                                                }
                                            }
                                            debug!("Successfully published records");
                                        }
                                    }
                                    Err(e) => error!("Failed to create drift records: {}", e),
                                }
                            }
                        }
                    },
                    _ = stop_rx.changed() => {
                        info!("Stopping background task");
                        if let Err(e) = producer.flush().await {
                            error!("Failed to flush producer: {}", e);
                        }
                        break;
                    }
                }
            }
        };

        let span = info_span!("background_task", task = %label);
        runtime.spawn(future.instrument(span));
        Ok(())
    }
}

/// This is a primary trait implemented on all queues
/// It provides the basic functionality for inserting, publishing, and flushing
pub trait QueueMethods {
    type ItemType: QueueExt + 'static;
    type FeatureQueue: FeatureQueue + 'static;

    /// These all need to be implemented in the concrete queue type
    fn capacity(&self) -> usize;
    fn get_runtime(&self) -> Arc<Runtime>;
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
    fn publish(&mut self, records: ServerRecords) -> Result<(), EventError> {
        let runtime = self.get_runtime();
        let producer = self.get_producer();

        runtime.block_on(async { producer.publish(records).await })
    }

    /// Insert an item into the queue
    fn insert(&mut self, item: Self::ItemType) -> Result<(), EventError> {
        let queue = self.queue();
        queue.push(item).map_err(EventError::queue_push_error)?;

        // Check if we need to process the queue
        if queue.is_full() {
            self.try_publish(queue)?;
        }

        Ok(())
    }

    /// Process the queue and publish records
    fn try_publish(&mut self, queue: Arc<ArrayQueue<Self::ItemType>>) -> Result<(), EventError> {
        let mut batch = Vec::with_capacity(queue.capacity());

        while let Some(metrics) = queue.pop() {
            batch.push(metrics);
        }

        if !batch.is_empty() {
            let feature_queue = self.feature_queue();
            match feature_queue.create_drift_records_from_batch(batch) {
                Ok(records) => {
                    self.publish(records)?;
                    self.update_last_publish()?;
                }
                Err(e) => error!("Failed to create drift records: {}", e),
            }
        }

        Ok(())
    }

    /// Flush the queue and shut down background tasks
    fn flush(&mut self) -> Result<(), EventError>;
}
