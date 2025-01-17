use pyo3::prelude::*;
use scouter_drift::spc::SpcFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::ScouterProducer;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::Features;
use tracing::error;

#[pyclass]
pub struct SpcQueue {
    queue: SpcFeatureQueue,
    producer: ScouterProducer,
    count: usize,
}

#[pymethods]
impl SpcQueue {
    #[new]
    #[pyo3(signature = (drift_profile, config, max_retries=None))]
    pub fn new(
        drift_profile: SpcDriftProfile,
        config: &Bound<'_, PyAny>,
        max_retries: Option<i32>,
    ) -> Result<Self, ScouterError> {
        Ok(SpcQueue {
            queue: SpcFeatureQueue::new(drift_profile),
            producer: ScouterProducer::new(config, max_retries)?,
            count: 0,
        })
    }

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
        self.producer.publish(records)?;
        self.queue.clear_queue();

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        self.producer.flush()
    }
}
