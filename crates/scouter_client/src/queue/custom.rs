use chrono::{NaiveDateTime, Utc};
use scouter_drift::custom::CustomMetricFeatureQueue;
use scouter_error::ScouterError;
use scouter_events::producer::RustScouterProducer;
use scouter_types::custom::CustomDriftProfile;
use scouter_types::Metrics;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{self, Duration};
use tracing::{debug, error, info, info_span, instrument, Instrument};

pub struct CustomQueue {
    queue: Arc<Mutex<CustomMetricFeatureQueue>>,
    producer: RustScouterProducer,
    count: usize,
    last_publish: NaiveDateTime,
    stop_tx: Option<watch::Sender<()>>,
    sample_size: usize,
    sample: bool,
    rt: Arc<tokio::runtime::Runtime>,
}

impl CustomQueue {
    pub fn new(
        drift_profile: CustomDriftProfile,
        producer: RustScouterProducer,
        rt: Arc<tokio::runtime::Runtime>,
    ) -> Result<Self, ScouterError> {
        let sample = drift_profile.config.sample;
        let sample_size = drift_profile.config.sample_size;

        debug!("Creating Custom Metric Queue");
        let queue = Arc::new(Mutex::new(CustomMetricFeatureQueue::new(drift_profile)));

        let (stop_tx, stop_rx) = watch::channel(());

        let custom_queue = CustomQueue {
            queue: queue.clone(),
            producer,
            count: 0,
            last_publish: Utc::now().naive_utc(),
            stop_tx: Some(stop_tx),
            sample_size,
            sample,
            rt,
        };

        debug!("Starting Background Task");
        custom_queue.start_background_task(queue, stop_rx)?;

        Ok(custom_queue)
    }

    #[instrument(skip_all, name = "Custom insert", level = "debug")]
    pub async fn insert(&mut self, metrics: Metrics) -> Result<(), ScouterError> {
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
        let mut queue = self.queue.blocking_lock();
        let records = queue.create_drift_records()?;

        if !records.records.is_empty() {
            self.producer.publish(records).await?;
            queue.clear_queue();
            self.last_publish = Utc::now().naive_utc();
        }

        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), ScouterError> {
        // publish any remaining drift records
        self._publish().await?;

        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.producer.flush().await
    }

    fn start_background_task(
        &self,
        queue: Arc<Mutex<CustomMetricFeatureQueue>>,
        mut stop_rx: watch::Receiver<()>,
    ) -> Result<(), ScouterError> {
        let queue = queue.clone();
        let mut producer = self.producer.clone();
        let mut last_publish = self.last_publish;
        let handle = self.rt.clone();

        // spawn the background task using the already cloned handle
        let future = async move {
            loop {
                tokio::select! {

                    _ = time::sleep(Duration::from_secs(2)) => {

                        debug!("Checking for records");

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

        handle.spawn(future.instrument(info_span!("Custom Background Polling")));

        Ok(())
    }
}
