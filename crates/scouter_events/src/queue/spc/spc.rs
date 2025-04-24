use crate::producer::RustScouterProducer;
use crate::queue::spc::feature_queue::SpcFeatureQueue;
use crate::queue::types::TransportConfig;
use pyo3::prelude::*;
use scouter_error::{EventError, ScouterError};
use scouter_types::Features;
use scouter_types::{spc::SpcDriftProfile, DriftProfile};
use std::sync::Arc;
use tracing::{debug, error, instrument};

pub struct SpcQueue {
    queue: SpcFeatureQueue,
    producer: RustScouterProducer,
    count: usize,
    rt: Arc<tokio::runtime::Runtime>,
}

impl SpcQueue {
    pub fn new(
        drift_profile: SpcDriftProfile,
        config: TransportConfig,
    ) -> Result<Self, EventError> {
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let producer = rt.block_on(async { RustScouterProducer::new(config).await })?;

        Ok(SpcQueue {
            queue: SpcFeatureQueue::new(drift_profile),
            producer,
            count: 0,
            rt,
        })
    }

    #[instrument(skip(self, features), name = "SPC Insert", level = "debug")]
    pub fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
        let insert = self.queue.insert(features);

        // silently fail if insert fails
        if insert.is_err() {
            error!(
                "Failed to insert features into queue: {:?}",
                insert.unwrap_err().to_string()
            );
            return Ok(());
        }

        self.count += 1;

        debug!(
            "count: {}, sample_size: {}",
            self.count, self.queue.drift_profile.config.sample_size
        );

        if self.count >= self.queue.drift_profile.config.sample_size {
            let publish = self._publish();

            // silently fail if publish fails
            if publish.is_err() {
                // log error as string
                error!(
                    "Failed to publish drift records: {:?}",
                    publish.unwrap_err().to_string()
                );
                return Ok(());
            }

            self.count = 0;
        }

        Ok(())
    }

    fn _publish(&mut self) -> Result<(), ScouterError> {
        let records = self.queue.create_drift_records()?;
        self.rt
            .block_on(async { self.producer.publish(records).await })?;
        self.queue.clear_queue();

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        Ok(self.rt.block_on(async { self.producer.flush().await })?)
    }
}
