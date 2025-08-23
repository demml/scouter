use crate::error::EventError;
use crate::producer::RustScouterProducer;
use crate::queue::spc::feature_queue::SpcFeatureQueue;
use crate::queue::traits::QueueMethods;
use crate::queue::types::TransportConfig;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use crossbeam_queue::ArrayQueue;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::Features;
use std::sync::Arc;
use std::sync::RwLock;

pub struct SpcQueue {
    queue: Arc<ArrayQueue<Features>>,
    feature_queue: Arc<SpcFeatureQueue>,
    producer: RustScouterProducer,
    last_publish: Arc<RwLock<DateTime<Utc>>>,
    capacity: usize,
}

impl SpcQueue {
    pub async fn new(
        drift_profile: SpcDriftProfile,
        config: TransportConfig,
    ) -> Result<Self, EventError> {
        let sample_size = drift_profile.config.sample_size;
        let queue = Arc::new(ArrayQueue::new(sample_size * 2)); // Add extra space for buffer
        let feature_queue = Arc::new(SpcFeatureQueue::new(drift_profile));
        let last_publish: Arc<RwLock<DateTime<Utc>>> = Arc::new(RwLock::new(Utc::now()));
        let producer = RustScouterProducer::new(config).await?;

        println!("Created SPC Queue with capacity: {}", sample_size);

        Ok(SpcQueue {
            queue,
            feature_queue,
            producer,
            last_publish,
            capacity: sample_size,
        })
    }
}

#[async_trait]
impl QueueMethods for SpcQueue {
    type ItemType = Features;
    type FeatureQueue = SpcFeatureQueue;

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
        self.producer.flush().await?;
        Ok(())
    }
}
