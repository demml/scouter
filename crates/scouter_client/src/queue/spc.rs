use pyo3::prelude::*;
use scouter_drift::spc::SpcFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::RustScouterProducer;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::Features;
use std::sync::Arc;
use tracing::{debug, error, info, span, Level};

#[pyclass]
pub struct SpcQueue {
    queue: SpcFeatureQueue,
    producer: RustScouterProducer,
    count: usize,
    rt: Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl SpcQueue {
    #[new]
    #[pyo3(signature = (drift_profile, config))]
    pub fn new(
        drift_profile: SpcDriftProfile,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        let span = span!(Level::INFO, "Creating SPC Queue").entered();
        let _ = span.enter();

        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let producer = rt.block_on(async { RustScouterProducer::new(config).await })?;

        info!("Starting up SpcQueue");

        Ok(SpcQueue {
            queue: SpcFeatureQueue::new(drift_profile),
            producer,
            count: 0,
            rt,
        })
    }

    pub fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
        let span = span!(Level::INFO, "SpcQueue Insert").entered();
        let _ = span.enter();

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
        let span = span!(Level::INFO, "SpcQueue Publish").entered();
        let _ = span.enter();
        debug!("Publishing drift records");

        let records = self.queue.create_drift_records()?;
        self.rt
            .block_on(async { self.producer.publish(records).await })?;
        self.queue.clear_queue();

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        let span = span!(Level::INFO, "SpcQueue Flush").entered();
        let _ = span.enter();
        debug!("Flushing SpcQueue");
        self.rt.block_on(async { self.producer.flush().await })
    }
}
