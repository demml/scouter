use crate::producer::RustScouterProducer;
use crate::queue::custom::feature_queue::CustomMetricFeatureQueue;
use crate::queue::traits::{BackgroundTask, FeatureQueue};
use crate::queue::types::TransportConfig;
use chrono::{DateTime, Utc};
use crossbeam_queue::SegQueue;
use scouter_error::EventError;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::Metrics;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::watch;
use tracing::{debug, error, instrument};

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
    metrics_queue: Arc<SegQueue<Metrics>>,
    feature_queue: Arc<CustomMetricFeatureQueue>,
    producer: RustScouterProducer,
    count: Arc<AtomicUsize>,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    stop_tx: Option<watch::Sender<()>>,
    rt: Arc<tokio::runtime::Runtime>,
    sample_size: usize,
    sample: bool,
}

impl BackgroundTask for CustomQueue {
    type DataItem = Metrics;
    type Processor = CustomMetricFeatureQueue;
}

impl CustomQueue {
    pub fn new(
        drift_profile: CustomDriftProfile,
        config: TransportConfig,
    ) -> Result<Self, EventError> {
        let sample = drift_profile.config.sample;
        let sample_size = drift_profile.config.sample_size;

        debug!("Creating Custom Metric Queue");
        let metrics_queue = Arc::new(SegQueue::new());
        let feature_queue = Arc::new(CustomMetricFeatureQueue::new(drift_profile));
        let count = Arc::new(AtomicUsize::new(0));
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
            metrics_queue: metrics_queue.clone(),
            feature_queue: feature_queue.clone(),
            producer,
            count,
            last_publish,
            stop_tx: Some(stop_tx),
            rt: rt.clone(),
            sample_size,
            sample,
        };

        debug!("Starting Background Task");
        custom_queue.start_background_worker(metrics_queue, feature_queue, stop_rx)?;

        Ok(custom_queue)
    }

    fn start_background_worker(
        &self,
        metrics_queue: Arc<SegQueue<Metrics>>,
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
            1000,
            "Custom Background Polling",
        )
    }

    #[instrument(skip_all)]
    pub fn insert(&mut self, metrics: Metrics) -> Result<(), EventError> {
        debug!("Inserting features");
        // Non-blocking push to queue
        self.metrics_queue.push(metrics);

        let current_count = self.count.fetch_add(1, Ordering::SeqCst);
        if current_count >= self.sample_size && self.sample {
            self.try_publish()?;
        }

        Ok(())
    }

    fn try_publish(&mut self) -> Result<(), EventError> {
        let mut batch = Vec::with_capacity(self.sample_size);

        // Drain the queue non-blockingly
        while let Some(metrics) = self.metrics_queue.pop() {
            batch.push(metrics);
        }

        if !batch.is_empty() {
            match self.feature_queue.create_drift_records_from_batch(batch) {
                Ok(records) => {
                    self.rt
                        .block_on(async { self.producer.publish(records).await })?;

                    if let Ok(mut last_publish) = self.last_publish.write() {
                        *last_publish = Utc::now();
                    }
                    self.count.store(0, Ordering::SeqCst);
                }
                Err(e) => error!("Failed to create drift records: {}", e),
            }
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), EventError> {
        // publish any remaining drift records
        self.try_publish()?;

        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        Ok(self.rt.block_on(async { self.producer.flush().await })?)
    }
}
