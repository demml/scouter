use chrono::{NaiveDateTime, Utc};
use pyo3::prelude::*;
use scouter_drift::psi::PsiFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::ScouterProducer;
use scouter_types::psi::PsiDriftProfile;
use scouter_types::Features;
use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};
use tracing::error;

const PSI_MAX_QUEUE_SIZE: usize = 1000;

#[pyclass]
pub struct PsiQueue {
    queue: Arc<Mutex<PsiFeatureQueue>>,
    producer: ScouterProducer,
    count: usize,
    last_publish: NaiveDateTime,
}

#[pymethods]
impl PsiQueue {
    #[new]
    pub fn new(
        drift_profile: PsiDriftProfile,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        let queue = Arc::new(Mutex::new(PsiFeatureQueue::new(drift_profile)));
        let producer = ScouterProducer::new(config)?;

        let psi_queue = PsiQueue {
            queue: queue.clone(),
            producer,
            count: 0,
            last_publish: Utc::now().naive_utc(),
        };

        psi_queue.start_background_task(queue)?;

        Ok(psi_queue)
    }

    pub fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
        {
            let mut queue = self.queue.lock().unwrap();
            let insert = queue.insert(features);

            // silently fail if insert fails
            if insert.is_err() {
                error!(
                    "Failed to insert features into queue: {:?}",
                    insert.unwrap_err().to_string()
                );
                return Ok(());
            }

            self.count += 1;
        }

        if self.count >= PSI_MAX_QUEUE_SIZE {
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
        let mut queue = self.queue.lock().unwrap();
        let records = queue.create_drift_records()?;

        if !records.records.is_empty() {
            self.producer.publish(records)?;
            queue.clear_queue();
            self.last_publish = Utc::now().naive_utc();
        }

        Ok(())
    }
}

impl PsiQueue {
    fn start_background_task(
        &self,
        queue: Arc<Mutex<PsiFeatureQueue>>,
    ) -> Result<(), ScouterError> {
        let queue = queue.clone();
        let mut producer = self.producer.clone();
        let last_publish = self.last_publish;

        // clone the handle
        let handle = producer.rt.clone();

        // spawn the background task using the already cloned handle
        handle.spawn(async move {
            loop {
                time::sleep(Duration::from_secs(30)).await;

                let now = Utc::now().naive_utc();
                let elapsed = now - last_publish;

                if elapsed.num_seconds() >= 30 {
                    let mut queue = match queue.lock() {
                        Ok(queue) => queue,
                        Err(e) => {
                            error!("Failed to lock queue: {:?}", e.to_string());
                            continue;
                        }
                    };

                    let records = match queue.create_drift_records() {
                        Ok(records) => records,
                        Err(e) => {
                            error!("Failed to create drift records: {:?}", e.to_string());
                            continue;
                        }
                    };

                    // no records to publish
                    if records.records.is_empty() {
                        continue;
                    }

                    if let Err(e) = producer.publish(records) {
                        error!("Failed to publish drift records: {:?}", e.to_string());
                        continue;
                    }

                    queue.clear_queue();
                }
            }
        });

        Ok(())
    }
}
