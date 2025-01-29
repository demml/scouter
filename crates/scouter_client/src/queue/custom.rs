use chrono::{NaiveDateTime, Utc};
use pyo3::prelude::*;
use scouter_drift::custom::CustomMetricFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::RustScouterProducer;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::Metrics;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{self, Duration};
use tracing::{debug, error, info, span, Instrument, Level};


#[pyclass]
pub struct CustomQueue {
    queue: Arc<Mutex<CustomMetricFeatureQueue>>,
    producer: RustScouterProducer,
    count: usize,
    last_publish: NaiveDateTime,
    stop_tx: Option<watch::Sender<()>>,
    rt: Arc<tokio::runtime::Runtime>,
    sample_size: usize,
    sample: bool,
}

#[pymethods]
impl CustomQueue {
    #[new]
    #[pyo3(signature = (drift_profile, config))]
    pub fn new(
        drift_profile: CustomDriftProfile,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        let span = span!(Level::INFO, "Custom Metric Queue").entered();
        let _ = span.enter();

        let sample = drift_profile.config.sample;
        let sample_size = drift_profile.config.sample_size;

        debug!("Creating Custom Metric Queue");
        let queue = Arc::new(Mutex::new(CustomMetricFeatureQueue::new(drift_profile)));

        // psi queue needs a tokio runtime to run background tasks
        // This runtime needs to be separate from the producer runtime
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

        debug!("Creating Producer");
        let producer = rt.block_on(async { RustScouterProducer::new(config).await })?;

        let (stop_tx, stop_rx) = watch::channel(());



        let custom_queue = CustomQueue {
            queue: queue.clone(),
            producer,
            count: 0,
            last_publish: Utc::now().naive_utc(),
            stop_tx: Some(stop_tx),
            rt: rt.clone(),
            sample_size,
            sample,
        };

        span.exit();

        debug!("Starting Background Task");
        custom_queue.start_background_task(queue, stop_rx)?;

        Ok(custom_queue)
    }

    pub fn insert(&mut self, metrics: Metrics) -> Result<(), ScouterError> {
        let span = span!(Level::INFO, "CustomQueue Insert").entered();
        let _ = span.enter();
        debug!("Inserting features");
        {
            let mut queue = self.queue.blocking_lock();
            let insert = queue.insert(metrics);

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

        if self.count >= self.sample_size && self.sample {
            debug!("Queue is full, publishing drift records");
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
            self.rt
                .block_on(async { self.producer.publish(records).await })?;
            queue.clear_queue();
            self.last_publish = Utc::now().naive_utc();
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.rt.block_on(async { self.producer.flush().await })
    }
}

impl CustomQueue {
    fn start_background_task(
        &self,
        queue: Arc<Mutex<CustomMetricFeatureQueue>>,
        mut stop_rx: watch::Receiver<()>,
    ) -> Result<(), ScouterError> {
        let queue = queue.clone();
        let mut producer = self.producer.clone();
        let mut last_publish = self.last_publish;
        let handle = self.rt.clone();

        let span = tracing::span!(tracing::Level::INFO, "Background Polling");

        let _ = span.enter();

        // spawn the background task using the already cloned handle
        let future = async move {
            loop {
                tokio::select! {

                    _ = time::sleep(Duration::from_secs(2)) => {

                        debug!("Checking if drift records need to be published");

                        let now = Utc::now().naive_utc();
                        let elapsed = now - last_publish;

                        if elapsed.num_seconds() >= 30 {
                            debug!("Locking queue");
                            let mut queue = queue.lock().await;

                            debug!("Creating drift records");
                            let records = match queue.create_drift_records() {
                                Ok(records) => records,
                                Err(e) => {
                                    error!("Failed to create drift records: {:?}", e.to_string());
                                    continue;
                                }
                            };

                            match !records.is_empty() {
                                true => {
                                    debug!("Publishing drift records");
                                    if let Err(e) = producer.publish(records).await {
                                        error!("Failed to publish drift records: {:?}", e.to_string());
                                    }
                                }
                                false => {
                                    debug!("No drift records to publish");
                                }
                            }

                            queue.clear_queue();
                            last_publish = now;
                        }
                    },
                    _ = stop_rx.changed() => {
                        info!("Stopping background task");
                        if let Err(e) = producer.flush().await {
                            error!("Failed to flush producer: {:?}", e.to_string());
                        }
                        break;
                    }
                }
            }
        };

        handle.spawn(future.instrument(span));

        Ok(())
    }
}
