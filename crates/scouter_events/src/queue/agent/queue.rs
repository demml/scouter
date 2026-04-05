use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::agent::record_queue::EvalRecordQueue;
use crate::queue::bus::{Event, TaskState};
use crate::queue::traits::BackgroundTask;
use crate::queue::traits::QueueMethods;
use crate::queue::types::QueueSettings;
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::agent::AgentEvalProfile;
use scouter_types::EvalRecord;
use std::sync::Arc;
use std::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::debug;

const GENAI_MAX_QUEUE_SIZE: usize = 25;

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
    queue: Arc<ArrayQueue<EvalRecord>>,
    record_queue: Arc<EvalRecordQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    capacity: usize,
    settings: Arc<RwLock<QueueSettings>>,
}

impl GenAIQueue {
    pub async fn new(
        drift_profile: AgentEvalProfile,
        config: TransportConfig,
        settings: Arc<RwLock<QueueSettings>>,
        task_state: &mut TaskState<Event>,
        identifier: String,
    ) -> Result<Self, EventError> {
        debug!("Creating GenAI Drift Queue");
        // ArrayQueue size is based on sample rate
        let queue = Arc::new(ArrayQueue::new(GENAI_MAX_QUEUE_SIZE * 2));
        let record_queue = Arc::new(EvalRecordQueue::new(drift_profile));
        let last_publish = Arc::new(RwLock::new(Utc::now()));

        let producer = RustScouterProducer::new(config).await?;
        let cancellation_token = CancellationToken::new();

        let genai_queue = GenAIQueue {
            queue: queue.clone(),
            record_queue: record_queue.clone(),
            producer,
            last_publish,
            capacity: GENAI_MAX_QUEUE_SIZE,
            settings,
        };

        let handle = genai_queue.start_background_task(
            queue,
            record_queue,
            genai_queue.producer.clone(),
            genai_queue.last_publish.clone(),
            genai_queue.capacity,
            identifier,
            task_state.clone(),
            cancellation_token.clone(),
        )?;

        task_state.add_background_abort_handle(handle);
        task_state.add_background_cancellation_token(cancellation_token);

        Ok(genai_queue)
    }

    pub fn should_insert(&self) -> bool {
        // if the sample rate is 1, we always insert
        let settings_read = self.settings.read().unwrap();
        let sample_ratio_percentage = settings_read.sample_ratio;

        if sample_ratio_percentage == 1.0 {
            return true;
        }
        // otherwise, we use the sample rate to determine if we should insert
        rand::random::<f64>() < sample_ratio_percentage
    }
}

impl BackgroundTask for GenAIQueue {
    type DataItem = EvalRecord;
    type Processor = EvalRecordQueue;
}

#[async_trait]
/// Implementing primary methods
impl QueueMethods for GenAIQueue {
    type ItemType = EvalRecord;
    type FeatureQueue = EvalRecordQueue;

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
