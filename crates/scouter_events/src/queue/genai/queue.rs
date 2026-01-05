use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::genai::record_queue::GenAIRecordQueue;
use crate::queue::traits::BackgroundTask;
use crate::queue::traits::QueueMethods;
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::genai::GenAIEvalProfile;
use scouter_types::GenAIEvalRecord;
use std::sync::Arc;
use std::sync::RwLock;
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
pub struct GenAIQueue {
    queue: Arc<ArrayQueue<GenAIEvalRecord>>,
    record_queue: Arc<GenAIRecordQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    capacity: usize,
    sample_rate_percentage: f64,
}

impl GenAIQueue {
    pub async fn new(
        drift_profile: GenAIEvalProfile,
        config: TransportConfig,
    ) -> Result<Self, EventError> {
        let sample_rate = drift_profile.config.sample_rate;

        // calculate sample rate percentage (1 / sample_rate)
        let sample_rate_percentage = 1.0 / sample_rate as f64;

        debug!("Creating GenAI Drift Queue");
        // ArrayQueue size is based on sample rate
        let queue = Arc::new(ArrayQueue::new(sample_rate * 2));
        let record_queue = Arc::new(GenAIRecordQueue::new());
        let last_publish = Arc::new(RwLock::new(Utc::now()));

        let producer = RustScouterProducer::new(config).await?;

        let genai_queue = GenAIQueue {
            queue,
            record_queue,
            producer,
            last_publish,
            capacity: sample_rate,
            sample_rate_percentage,
        };

        Ok(genai_queue)
    }

    pub fn should_insert(&self) -> bool {
        // if the sample rate is 1, we always insert
        if self.sample_rate_percentage == 1.0 {
            return true;
        }
        // otherwise, we use the sample rate to determine if we should insert
        rand::random::<f64>() < self.sample_rate_percentage
    }
}

impl BackgroundTask for GenAIQueue {
    type DataItem = GenAIEvalRecord;
    type Processor = GenAIRecordQueue;
}

#[async_trait]
/// Implementing primary methods
impl QueueMethods for GenAIQueue {
    type ItemType = GenAIEvalRecord;
    type FeatureQueue = GenAIRecordQueue;

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
        self.record_queue.clone()
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

        self.producer.flush().await?;
        Ok(())
    }
}
