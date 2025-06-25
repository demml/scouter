#![allow(clippy::useless_conversion)]
use crate::error::{EventError, PyEventError};
use crate::queue::bus::{Event, QueueBus};
use crate::queue::custom::CustomQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::QueueMethods;
use crate::queue::types::TransportConfig;
use pyo3::prelude::*;
use scouter_types::{DriftProfile, QueueItem};
use scouter_types::{Features, Metrics};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;
use tracing::{debug, error, instrument};

pub enum QueueNum {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
}

impl QueueNum {
    pub async fn new(
        drift_profile: DriftProfile,
        config: TransportConfig,
        queue_runtime: Arc<tokio::runtime::Runtime>,
    ) -> Result<Self, EventError> {
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, config).await?;
                Ok(QueueNum::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue = PsiQueue::new(psi_profile, config, queue_runtime).await?;
                Ok(QueueNum::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue = CustomQueue::new(custom_profile, config, queue_runtime).await?;
                Ok(QueueNum::Custom(queue))
            }
        }
    }

    /// Top-level insert method for the queue
    /// This method will take a QueueItem and insert it into the appropriate queue
    /// If features, inserts using insert_features (spc, psi)
    /// If metrics, inserts using insert_metrics (custom)
    ///
    /// # Arguments
    /// * `entity` - The entity to insert into the queue
    #[instrument(skip_all)]
    pub async fn insert(&mut self, entity: QueueItem) -> Result<(), EventError> {
        debug!("Inserting entity into queue: {:?}", entity);
        match entity {
            QueueItem::Features(features) => self.insert_features(features).await,
            QueueItem::Metrics(metrics) => self.insert_metrics(metrics).await,
        }
    }

    /// Insert features into the queue. Currently only applies to PSI and SPC queues
    ///
    /// # Arguments
    /// * `features` - The features to insert into the queue
    ///
    ///
    #[instrument(skip_all)]
    pub async fn insert_features(&mut self, features: Features) -> Result<(), EventError> {
        match self {
            QueueNum::Psi(queue) => queue.insert(features).await,
            QueueNum::Spc(queue) => queue.insert(features).await,
            _ => Err(EventError::QueueNotSupportedFeatureError),
        }
    }

    /// Insert metrics into the queue. Currently only applies to custom queues
    ///
    /// # Arguments
    /// * `metrics` - The metrics to insert into the queue
    ///
    pub async fn insert_metrics(&mut self, metrics: Metrics) -> Result<(), EventError> {
        match self {
            QueueNum::Custom(queue) => queue.insert(metrics).await,
            _ => Err(EventError::QueueNotSupportedMetricsError),
        }
    }

    /// Flush the queue. This will publish the records to the producer
    /// and shut down the background tasks
    pub async fn flush(&mut self) -> Result<(), EventError> {
        match self {
            QueueNum::Spc(queue) => queue.flush().await,
            QueueNum::Psi(queue) => queue.flush().await,
            QueueNum::Custom(queue) => queue.flush().await,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_queue_events(
    mut rx: UnboundedReceiver<Event>,
    mut shutdown_rx: oneshot::Receiver<()>,
    drift_profile: DriftProfile,
    config: TransportConfig,
    id: String,
    queue_runtime: Arc<tokio::runtime::Runtime>,
    completion_tx: oneshot::Sender<()>,
) -> Result<(), EventError> {
    let mut queue = match QueueNum::new(drift_profile, config.clone(), queue_runtime).await {
        Ok(q) => q,
        Err(e) => {
            error!("Failed to initialize queue {}: {}", id, e);
            return Err(e);
        }
    };
    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                match event {
                    Event::Task(entity) => {
                        match queue.insert(entity).await {
                            Ok(_) => {
                                debug!("Inserted entity into queue {}", id);
                            }
                            Err(e) => {
                                error!("Error inserting entity into queue {}: {}", id, e);
                            }
                        }
                    }
                }
            }
            _ = &mut shutdown_rx => {
                debug!("Shutdown signal received for queue {}", id);
                queue.flush().await?;
                completion_tx.send(()).map_err(|_| EventError::SignalCompletionError)?;
                break;
            }
        }
    }
    Ok(())
}

#[pyclass]
pub struct ScouterQueue {
    queues: HashMap<String, Py<QueueBus>>,
    _shared_runtime: Arc<tokio::runtime::Runtime>,
    completion_rxs: HashMap<String, oneshot::Receiver<()>>,
}

#[pymethods]
impl ScouterQueue {
    /// Create a new ScouterQueue from a map of aliases and paths
    /// This will create a new ScouterQueue for each path in the map
    ///
    /// # Process
    /// 1. Create empty queues
    /// 2. Extract transport config from python object
    /// 3. Create a shared tokio runtime that is used to create background queues
    /// 4. For each path in the map, create a new queue
    /// 5. Spawn a new thread for each queue (some queues require background tasks)
    /// 6. Return the ScouterQueue
    ///
    /// # Arguments
    /// * `paths` - A map of aliases to paths
    /// * `transport_config` - The transport config to use
    ///
    /// # Returns
    /// * `ScouterQueue` - A new ScouterQueue
    #[staticmethod]
    #[pyo3(signature = (path, transport_config))]
    pub fn from_path(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
    ) -> Result<Self, PyEventError> {
        // create a tokio runtime to run the background tasks
        let shared_runtime =
            Arc::new(tokio::runtime::Runtime::new().map_err(EventError::SetupTokioRuntimeError)?);

        ScouterQueue::from_path_rs(py, path, transport_config, shared_runtime)
    }

    /// Get a queue by its alias
    ///
    /// # Example
    /// ```python
    /// from scouter import ScouterQueue
    ///
    /// scouter_queues = ScouterQueue.from_path(...)
    /// scouter_queues["queue_alias"].insert(features)
    /// ```
    pub fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &str,
    ) -> Result<&Bound<'py, QueueBus>, PyEventError> {
        match self.queues.get(key) {
            Some(queue) => Ok(queue.bind(py)),
            None => Err(PyEventError::MissingQueueError(key.to_string())),
        }
    }

    /// Triggers a global shutdown for all queues
    ///
    /// # Example
    ///
    /// ```python
    /// from scouter import ScouterQueue
    ///
    /// scouter_queues = ScouterQueue.from_path(...)
    /// scouter_queues.shutdown()
    ///
    /// ```
    pub fn shutdown(&mut self, py: Python) -> Result<(), PyEventError> {
        // trigger shutdown for all queues
        for queue in self.queues.values() {
            let bound = queue.bind(py);
            bound
                .call_method0("shutdown")
                .map_err(PyEventError::ShutdownQueueError)?;
        }

        self._shared_runtime.block_on(async {
            for (id, completion_rx) in self.completion_rxs.drain() {
                completion_rx
                    .await
                    .map_err(EventError::ShutdownReceiverError)?;
                debug!("Queue {} initialized successfully", id);
            }
            Ok::<_, PyEventError>(())
        })?;

        self.queues.clear();

        Ok(())
    }
}

impl ScouterQueue {
    /// Create a new ScouterQueue from a map of aliases and paths
    /// This will create a new ScouterQueue for each path in the map
    /// This method was created to help with integration into the Opsml CardDeck where this
    /// method is called directly.
    ///
    /// # Process
    /// 1. Create empty queues
    /// 2. Extract transport config from python object
    /// 3. Create a shared tokio runtime that is used to create background queues
    /// 4. For each path in the map, create a new queue
    /// 5. Spawn a new thread for each queue (some queues require background tasks)
    /// 6. Return the ScouterQueue
    ///
    /// # Arguments
    /// * `paths` - A map of aliases to paths
    /// * `transport_config` - The transport config to use
    /// * *shared_runtime* - A shared tokio runtime that is used to create background queues
    ///
    /// # Returns
    /// * `ScouterQueue` - A new ScouterQueue
    pub fn from_path_rs(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
        shared_runtime: Arc<tokio::runtime::Runtime>,
    ) -> Result<Self, PyEventError> {
        debug!("Creating ScouterQueue from path");
        let mut queues = HashMap::new();
        //let mut startup_rxs = Vec::new();
        let mut completion_rxs = HashMap::new();

        // assert transport config is not None
        if transport_config.is_none() {
            return Err(PyEventError::MissingTransportConfig);
        }

        // Extract transport config from python object
        let config = TransportConfig::from_py_config(transport_config)?;

        // load each profile from path
        // In practice you can load as many profiles as you want
        for (id, profile_path) in path {
            let cloned_config = config.clone();
            let drift_profile = DriftProfile::from_profile_path(profile_path)?;

            // create startup channels to ensure queues are initialized before use
            //let (startup_tx, startup_rx) = oneshot::channel();

            // create completion channels to ensure queues are flushed before shutdown
            let (completion_tx, completion_rx) = oneshot::channel();
            let (bus, rx, shutdown_rx) = QueueBus::new();

            let queue_runtime = shared_runtime.clone();

            // spawn a new thread for each queue
            let id_clone = id.clone();

            // Just spawn the task without waiting for initialization
            shared_runtime.spawn(async move {
                match handle_queue_events(
                    rx,
                    shutdown_rx,
                    drift_profile,
                    cloned_config,
                    id_clone,
                    queue_runtime,
                    completion_tx,
                )
                .await
                {
                    Ok(_) => debug!("Queue handler exited successfully"),
                    Err(e) => error!("Queue handler exited with error: {}", e),
                }
            });

            let queue = Py::new(py, bus)?;

            queues.insert(id.clone(), queue);
            //startup_rxs.push((id.clone(), startup_rx));
            completion_rxs.insert(id, completion_rx);
        }

        // wait for all queues to start up
        //shared_runtime.block_on(async {
        //    for (id, startup_rx) in startup_rxs {
        //        startup_rx.await.map_err(EventError::StartupReceiverError)?;
        //        debug!("Queue {} initialized successfully", id);
        //    }
        //    Ok::<_, EventError>(())
        //})?;

        Ok(ScouterQueue {
            queues,
            // need to keep the runtime alive for the life of ScouterQueue
            _shared_runtime: shared_runtime,
            completion_rxs,
        })
    }
}
