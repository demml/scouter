use crate::producer::RustScouterProducer;
use crate::queue::custom::feature_queue::CustomMetricFeatureQueue;
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
use tokio::time::{self, Duration};
use tracing::{debug, error, info, info_span, instrument, Instrument};

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
        custom_queue.start_background_task(metrics_queue, feature_queue, stop_rx)?;

        Ok(custom_queue)
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

    fn start_background_task(
        &self,
        metrics_queue: Arc<SegQueue<Metrics>>,
        feature_queue: Arc<CustomMetricFeatureQueue>,
        mut stop_rx: watch::Receiver<()>,
    ) -> Result<(), EventError> {
        let mut producer = self.producer.clone();
        let last_publish = self.last_publish.clone();
        let handle = self.rt.clone();

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
                            debug!("Processing queued metrics");

                            let mut batch = Vec::with_capacity(1000);
                            while let Some(metrics) = metrics_queue.pop() {
                                batch.push(metrics);
                            }

                            if !batch.is_empty() {
                                match feature_queue.create_drift_records_from_batch(batch) {
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

        handle.spawn(future.instrument(info_span!("Custom Background Polling")));
        Ok(())
    }
}
