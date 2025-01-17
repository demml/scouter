use chrono::{NaiveDateTime, Utc};
use pyo3::prelude::*;
use scouter_drift::psi::PsiFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::ScouterProducer;
use scouter_types::psi::PsiDriftProfile;
use scouter_types::Features;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{self, Duration};
use tracing::{error, info};

const PSI_MAX_QUEUE_SIZE: usize = 1000;

#[pyclass]
pub struct PsiQueue {
    queue: Arc<Mutex<PsiFeatureQueue>>,
    producer: ScouterProducer,
    count: usize,
    last_publish: NaiveDateTime,
    stop_tx: Option<watch::Sender<()>>,
    rt: Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl PsiQueue {
    #[new]
    #[pyo3(signature = (drift_profile, config))]
    pub fn new(
        drift_profile: PsiDriftProfile,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        let queue = Arc::new(Mutex::new(PsiFeatureQueue::new(drift_profile)));

        // psi queue needs a tokio runtime to run background tasks
        // This runtime needs to be separate from the producer runtime
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());
        
        let producer = ScouterProducer::new(config)?;

        let (stop_tx, stop_rx) = watch::channel(());

        let psi_queue = PsiQueue {
            queue: queue.clone(),
            producer,
            count: 0,
            last_publish: Utc::now().naive_utc(),
            stop_tx: Some(stop_tx),
            rt: rt.clone(),
        };

        psi_queue.start_background_task(queue, stop_rx)?;

        Ok(psi_queue)
    }

    pub fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
        {
            let mut queue = self.queue.blocking_lock();
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
        let mut queue = self.queue.blocking_lock();
        let records = queue.create_drift_records()?;

        if !records.records.is_empty() {
            self.producer.publish(records)?;
            queue.clear_queue();
            self.last_publish = Utc::now().naive_utc();
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.producer.flush()
    }
}

impl PsiQueue {
    fn start_background_task(
        &self,
        queue: Arc<Mutex<PsiFeatureQueue>>,
        mut stop_rx: watch::Receiver<()>,
    ) -> Result<(), ScouterError> {
        let queue = queue.clone();
        let mut producer = self.producer.clone();
        let last_publish = self.last_publish;
        let handle = self.rt.clone();

        // spawn the background task using the already cloned handle
        handle.spawn(async move {
            loop {
                tokio::select! {
                    _ = time::sleep(Duration::from_secs(2)) => {
                        let now = Utc::now().naive_utc();
                        let elapsed = now - last_publish;

                        if elapsed.num_seconds() >= 30 {
                            let mut queue = queue.lock().await;

                            let records = match queue.create_drift_records() {
                                Ok(records) => records,
                                Err(e) => {
                                    error!("Failed to create drift records: {:?}", e.to_string());
                                    continue;
                                }
                            };

                            if let Err(e) = producer.publish(records) {
                                error!("Failed to publish drift records: {:?}", e.to_string());
                                continue;
                            }

                            queue.clear_queue();
                        }
                    },
                    _ = stop_rx.changed() => {
                        info!("Stopping background task");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}
