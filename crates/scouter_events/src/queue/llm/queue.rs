use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::custom::feature_queue::CustomMetricFeatureQueue;
use crate::queue::llm::record_queue::LLMDriftRecordQueue;
use crate::queue::traits::BackgroundTask;
use crate::queue::traits::FeatureQueue;
use crate::queue::traits::QueueMethods;
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::llm::LLMDriftProfile;
use scouter_types::LLMRecord;
use scouter_types::Metrics;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime;
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
pub struct LLMQueue {
    queue: Arc<ArrayQueue<LLMRecord>>,
    drift_queue: Arc<LLMRecordQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    stop_tx: Option<watch::Sender<()>>,
    capacity: usize,
    sample_rate_percentage: f64,
}

impl LLMDriftQueue {
    pub async fn new(
        drift_profile: LLMDriftProfile,
        config: TransportConfig,
        runtime: Arc<runtime::Runtime>,
    ) -> Result<Self, EventError> {
        let sample_rate = drift_profile.config.sample_rate;

        // calculate sample rate percentage (1 / sample_rate)
        let sample_rate_percentage = 1.0 / sample_rate as f64;

        debug!("Creating LLM Drift Queue");
        // ArrayQueue size is based on sample rate
        let queue = Arc::new(ArrayQueue::new(sample_rate * 2));
        let llm_drift_queue = Arc::new(LLMDriftRecordQueue::new(drift_profile));
        let last_publish = Arc::new(RwLock::new(Utc::now()));

        debug!("Creating Producer");
        let producer = RustScouterProducer::new(config).await?;

        let (stop_tx, stop_rx) = watch::channel(());

        let llm_queue = LLMDriftQueue {
            queue: queue.clone(),
            drift_queue: llm_drift_queue.clone(),
            producer,
            last_publish,
            stop_tx: Some(stop_tx),
            capacity: sample_rate,
            sample_rate_percentage,
        };

        debug!("Starting Background Task");
        llm_queue.start_background_worker(queue, llm_drift_queue, stop_rx, runtime)?;

        Ok(llm_queue)
    }

    fn start_background_worker(
        &self,
        queue: Arc<ArrayQueue<LLMDriftRecord>>,
        drift_queue: Arc<LLMDriftRecordQueue>,
        stop_rx: watch::Receiver<()>,
        rt: Arc<tokio::runtime::Runtime>,
    ) -> Result<(), EventError> {
        self.start_background_task(
            queue,
            drift_queue,
            self.producer.clone(),
            self.last_publish.clone(),
            rt.clone(),
            stop_rx,
            self.capacity,
            "Custom Background Polling",
        )
    }
}

impl BackgroundTask for LLMDriftQueue {
    type DataItem = LLMDriftRecord;
    type Processor = LLMDriftRecordQueue;
}

#[async_trait]
/// Implementing primary methods
impl QueueMethods for LLMDriftQueue {
    type ItemType = LLMDriftRecord;
    type FeatureQueue = LLMDriftRecordQueue;

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
        self.drift_queue.clone()
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
        self.producer.flush().await
    }
}
