use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::bus::EventLoops;
use crate::queue::custom::feature_queue::CustomMetricFeatureQueue;
use crate::queue::traits::queue::wait_for_background_task;
use crate::queue::traits::{BackgroundTask, QueueMethods};
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::Metrics;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::debug;
/// The following code is a custom queue implementation for handling custom metrics.
/// It consists of a `CustomQueue` struct that manages a queue of metrics and a background task
///
/// components:
/// - `metrics_queue`: Crossbeam queue for storing metrics.
/// - `feature_queue`: A Custom metric helper for converting batches of metrics to drift records.
/// - `producer`: A RustScouter producer for publishing records.
/// - `count`: Atomic counter for tracking the number of metrics.
/// - `last_publish`: A timestamp for the last publish time.
/// - `stop_tx`: A channel for stopping the background task.
/// - `rt`: A Tokio runtime for executing asynchronous tasks.
/// - `sample_size`: The size of the sample.
/// - `sample`: A boolean indicating whether to sample metrics.
pub struct CustomQueue {
    queue: Arc<ArrayQueue<Metrics>>,
    feature_queue: Arc<CustomMetricFeatureQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    stop_tx: Option<watch::Sender<()>>,
    capacity: usize,
    event_loops: EventLoops,
}

impl CustomQueue {
    pub async fn new(
        drift_profile: CustomDriftProfile,
        config: TransportConfig,
        runtime: Arc<runtime::Runtime>,
        event_loops: &mut EventLoops,
    ) -> Result<Self, EventError> {
        let sample_size = drift_profile.config.sample_size;

        debug!("Creating Custom Metric Queue");
        // ArrayQueue size is based on sample size
        let metrics_queue = Arc::new(ArrayQueue::new(sample_size * 2));
        let feature_queue = Arc::new(CustomMetricFeatureQueue::new(drift_profile));
        let last_publish = Arc::new(RwLock::new(Utc::now()));

        debug!("Creating Producer");
        let producer = RustScouterProducer::new(config).await?;

        let (stop_tx, stop_rx) = watch::channel(());

        let custom_queue = CustomQueue {
            queue: metrics_queue.clone(),
            feature_queue: feature_queue.clone(),
            producer,
            last_publish,
            stop_tx: Some(stop_tx),
            capacity: sample_size,
            event_loops: event_loops.clone(),
        };

        debug!("Starting Background Task");
        let handle = custom_queue.start_background_task(
            metrics_queue,
            feature_queue,
            custom_queue.producer.clone(),
            custom_queue.last_publish.clone(),
            runtime.clone(),
            stop_rx,
            custom_queue.capacity,
            "Custom Background Polling",
            event_loops.clone(),
        )?;

        event_loops.add_event_handle(handle);
        //wait_for_background_task(event_loops)?;

        Ok(custom_queue)
    }
}

/// Custom requires a background timed-task as a secondary processing mechanism
/// i.e. Its possible that queue insertion is slow, and so we need a background
/// task to process the queue at a regular interval
impl BackgroundTask for CustomQueue {
    type DataItem = Metrics;
    type Processor = CustomMetricFeatureQueue;
}

#[async_trait]
/// Implementing primary methods
impl QueueMethods for CustomQueue {
    type ItemType = Metrics;
    type FeatureQueue = CustomMetricFeatureQueue;

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn get_producer(&mut self) -> &mut RustScouterProducer {
        &mut self.producer
    }

    fn queue(&self) -> Arc<ArrayQueue<Self::ItemType>> {
        self.queue.clone()
    }

    fn feature_queue(&self) -> Arc<Self::FeatureQueue> {
        self.feature_queue.clone()
    }

    fn last_publish(&self) -> Arc<RwLock<DateTime<Utc>>> {
        self.last_publish.clone()
    }

    fn should_process(&self, current_count: usize) -> bool {
        current_count >= self.capacity()
    }

    async fn flush(&mut self) -> Result<(), EventError> {
        // publish any remaining drift records
        self.try_publish(self.queue()).await?;
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.producer.flush().await?;

        self.event_loops.shutdown_background_task().await?;

        debug!("Custom Background Task finished");
        Ok(())
    }
}
