use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::bus::TaskState;
use crate::queue::psi::feature_queue::PsiFeatureQueue;
use crate::queue::traits::{BackgroundTask, QueueMethods};
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::psi::PsiDriftProfile;
use scouter_types::Features;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime;
use tokio_util::sync::CancellationToken;
use tracing::debug;

const PSI_MAX_QUEUE_SIZE: usize = 1000;

pub struct PsiQueue {
    queue: Arc<ArrayQueue<Features>>,
    feature_queue: Arc<PsiFeatureQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    capacity: usize,
}

impl PsiQueue {
    pub async fn new(
        drift_profile: PsiDriftProfile,
        config: TransportConfig,
        runtime: Arc<runtime::Runtime>,
        task_state: &mut TaskState,
        identifier: String,
    ) -> Result<Self, EventError> {
        // ArrayQueue size is based on the max PSI queue size

        let queue = Arc::new(ArrayQueue::new(PSI_MAX_QUEUE_SIZE * 2));
        let feature_queue = Arc::new(PsiFeatureQueue::new(drift_profile));
        let last_publish: Arc<RwLock<DateTime<Utc>>> = Arc::new(RwLock::new(Utc::now()));
        let producer = RustScouterProducer::new(config).await?;
        let cancellation_token = CancellationToken::new();

        let psi_queue = PsiQueue {
            queue: queue.clone(),
            feature_queue: feature_queue.clone(),
            producer,
            last_publish,
            capacity: PSI_MAX_QUEUE_SIZE,
        };

        let handle = psi_queue.start_background_task(
            queue,
            feature_queue,
            psi_queue.producer.clone(),
            psi_queue.last_publish.clone(),
            runtime.clone(),
            PSI_MAX_QUEUE_SIZE,
            identifier,
            task_state.clone(),
            cancellation_token.clone(),
        )?;

        task_state.add_background_abort_handle(handle);
        task_state.add_background_cancellation_token(cancellation_token);

        debug!("Created PSI Queue with capacity: {}", PSI_MAX_QUEUE_SIZE);

        Ok(psi_queue)
    }
}

/// Psi requires a background timed-task as a secondary processing mechanism
/// i.e. Its possible that queue insertion is slow, and so we need a background
/// task to process the queue at a regular interval
impl BackgroundTask for PsiQueue {
    type DataItem = Features;
    type Processor = PsiFeatureQueue;
}

#[async_trait]
/// Implementing primary methods
impl QueueMethods for PsiQueue {
    type ItemType = Features;
    type FeatureQueue = PsiFeatureQueue;

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
        self.producer.flush().await?;

        Ok(())
    }
}
