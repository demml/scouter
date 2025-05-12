use crate::error::EventError;
use crate::producer::RustScouterProducer;
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
use tokio::sync::watch;
use tracing::debug;

const PSI_MAX_QUEUE_SIZE: usize = 1000;

pub struct PsiQueue {
    queue: Arc<ArrayQueue<Features>>,
    feature_queue: Arc<PsiFeatureQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    stop_tx: Option<watch::Sender<()>>,
    capacity: usize,
}

impl PsiQueue {
    pub async fn new(
        drift_profile: PsiDriftProfile,
        config: TransportConfig,
        runtime: Arc<runtime::Runtime>,
    ) -> Result<Self, EventError> {
        // ArrayQueue size is based on the max PSI queue size

        let queue = Arc::new(ArrayQueue::new(PSI_MAX_QUEUE_SIZE * 2));
        let feature_queue = Arc::new(PsiFeatureQueue::new(drift_profile));
        let last_publish: Arc<RwLock<DateTime<Utc>>> = Arc::new(RwLock::new(Utc::now()));
        let producer = RustScouterProducer::new(config).await?;

        debug!("Creating PSI Queue");

        let (stop_tx, stop_rx) = watch::channel(());

        let psi_queue = PsiQueue {
            queue: queue.clone(),
            feature_queue: feature_queue.clone(),
            producer,
            last_publish,
            stop_tx: Some(stop_tx),
            capacity: PSI_MAX_QUEUE_SIZE,
        };

        debug!("Starting Background Task");
        psi_queue.start_background_worker(queue, feature_queue, stop_rx, runtime)?;

        Ok(psi_queue)
    }

    fn start_background_worker(
        &self,
        metrics_queue: Arc<ArrayQueue<Features>>,
        feature_queue: Arc<PsiFeatureQueue>,
        stop_rx: watch::Receiver<()>,
        rt: Arc<tokio::runtime::Runtime>,
    ) -> Result<(), EventError> {
        self.start_background_task(
            metrics_queue,
            feature_queue,
            self.producer.clone(),
            self.last_publish.clone(),
            rt.clone(),
            stop_rx,
            PSI_MAX_QUEUE_SIZE,
            "Psi Background Polling",
        )
    }
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
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.producer.flush().await
    }
}

/// Psi requires a background timed-task as a secondary processing mechanism
/// i.e. Its possible that queue insertion is slow, and so we need a background
/// task to process the queue at a regular interval
impl BackgroundTask for PsiQueue {
    type DataItem = Features;
    type Processor = PsiFeatureQueue;
}
