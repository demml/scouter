use pyo3::prelude::*;
use scouter_drift::spc::SpcFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::RustScouterProducer;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::Features;
use tracing::{debug, error, instrument};

pub struct SpcQueue {
    queue: SpcFeatureQueue,
    producer: RustScouterProducer,
    count: usize,
}

impl SpcQueue {
    pub async fn new(
        drift_profile: SpcDriftProfile,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        let producer = RustScouterProducer::new(config).await?;

        Ok(SpcQueue {
            queue: SpcFeatureQueue::new(drift_profile),
            producer,
            count: 0,
        })
    }

    #[instrument(skip_all, name = "Spc Insert", level = "debug")]
    pub async fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
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
            let publish = self._publish().await;

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

    async fn _publish(&mut self) -> Result<(), ScouterError> {
        let records = self.queue.create_drift_records()?;
        self.producer.publish(records).await?;
        self.queue.clear_queue();

        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), ScouterError> {
        self.producer.flush().await
    }
}
