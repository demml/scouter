use crate::producer::RustScouterProducer;
use crate::queue::custom::feature_queue::CustomMetricFeatureQueue;
use crate::queue::traits::BackgroundTask;
use crate::queue::traits::QueueMethods;
use crate::queue::types::TransportConfig;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_error::EventError;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::Metrics;
use std::cmp::min;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::watch;
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
    rt: Arc<tokio::runtime::Runtime>,
    sample_size: usize,
}

impl CustomQueue {
    pub fn new(
        drift_profile: CustomDriftProfile,
        config: TransportConfig,
    ) -> Result<Self, EventError> {
        let sample_size = drift_profile.config.sample_size;

        debug!("Creating Custom Metric Queue");
        // ArrayQueue size is based on sample size
        let metrics_queue = Arc::new(ArrayQueue::new(sample_size));
        let feature_queue = Arc::new(CustomMetricFeatureQueue::new(drift_profile));
        let last_publish = Arc::new(RwLock::new(Utc::now()));

        // psi queue needs a tokio runtime to run background tasks
        // This runtime needs to be separate from the producer runtime
        let rt = Arc::new(
            tokio::runtime::Runtime::new().map_err(EventError::traced_setup_runtime_error)?,
        );

        debug!("Creating Producer");
        let producer = rt.block_on(async { RustScouterProducer::new(config).await })?;

        let (stop_tx, stop_rx) = watch::channel(());

        let custom_queue = CustomQueue {
            queue: metrics_queue.clone(),
            feature_queue: feature_queue.clone(),
            producer,
            last_publish,
            stop_tx: Some(stop_tx),
            rt: rt.clone(),
            sample_size,
        };

        debug!("Starting Background Task");
        custom_queue.start_background_worker(metrics_queue, feature_queue, stop_rx)?;

        Ok(custom_queue)
    }

    fn start_background_worker(
        &self,
        metrics_queue: Arc<ArrayQueue<Metrics>>,
        feature_queue: Arc<CustomMetricFeatureQueue>,
        stop_rx: watch::Receiver<()>,
    ) -> Result<(), EventError> {
        self.start_background_task(
            metrics_queue,
            feature_queue,
            self.producer.clone(),
            self.last_publish.clone(),
            self.rt.clone(),
            stop_rx,
            min(self.sample_size, 1000),
            "Custom Background Polling",
        )
    }
}

impl BackgroundTask for CustomQueue {
    type DataItem = Metrics;
    type Processor = CustomMetricFeatureQueue;
}

/// Implementing primary methods
impl QueueMethods for CustomQueue {
    type ItemType = Metrics;
    type FeatureQueue = CustomMetricFeatureQueue;

    fn capacity(&self) -> usize {
        self.sample_size
    }

    fn get_runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.rt.clone()
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

    fn flush(&mut self) -> Result<(), EventError> {
        // publish any remaining drift records
        self.try_publish(self.queue())?;
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        Ok(self.rt.block_on(async { self.producer.flush().await })?)
    }
}
