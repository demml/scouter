#![allow(clippy::useless_conversion)]
use crate::error::{EventError, PyEventError};
use crate::queue::bus::{Event, EventLoops, QueueBus};
use crate::queue::custom::CustomQueue;
use crate::queue::llm::LLMQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::QueueMethods;
use crate::queue::types::TransportConfig;
use pyo3::prelude::*;
use scouter_types::{DriftProfile, LLMRecord, QueueItem};
use scouter_types::{Features, Metrics};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::runtime;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, error, info, instrument};
pub enum QueueNum {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
    LLM(LLMQueue),
}
// need to add queue running lock to each and return it to the queue bus
impl QueueNum {
    pub async fn new(
        transport_config: TransportConfig,
        drift_profile: DriftProfile,
        queue_running: Arc<RwLock<bool>>,
        runtime: Arc<runtime::Runtime>,
        background_loop: Arc<RwLock<Option<JoinHandle<()>>>>,
    ) -> Result<Self, EventError> {
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, transport_config).await?;
                Ok(QueueNum::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue = PsiQueue::new(
                    psi_profile,
                    transport_config,
                    runtime,
                    background_loop.clone(),
                )
                .await?;
                Ok(QueueNum::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue = CustomQueue::new(
                    custom_profile,
                    transport_config,
                    runtime,
                    background_loop.clone(),
                )
                .await?;
                Ok(QueueNum::Custom(queue))
            }
            DriftProfile::LLM(llm_profile) => {
                let queue = LLMQueue::new(llm_profile, transport_config).await?;
                Ok(QueueNum::LLM(queue))
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
            QueueItem::LLM(llm_record) => self.insert_llm_record(*llm_record).await,
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

    /// Insert LLM record into the queue. Currently only applies to LLM queues
    ///
    /// # Arguments
    /// * `llm_record` - The LLM record to insert into the queue
    ///
    pub async fn insert_llm_record(&mut self, llm_record: LLMRecord) -> Result<(), EventError> {
        match self {
            QueueNum::LLM(queue) => {
                if !queue.should_insert() {
                    debug!("Skipping LLM record insertion due to sampling rate");
                    return Ok(());
                }
                queue.insert(llm_record).await
            }
            _ => Err(EventError::QueueNotSupportedLLMError),
        }
    }

    /// Flush the queue. This will publish the records to the producer
    /// and shut down the background tasks
    pub async fn flush(&mut self) -> Result<(), EventError> {
        match self {
            QueueNum::Spc(queue) => queue.flush().await,
            QueueNum::Psi(queue) => queue.flush().await,
            QueueNum::Custom(queue) => queue.flush().await,
            QueueNum::LLM(queue) => queue.flush().await,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_queue_events(
    mut rx: UnboundedReceiver<Event>,
    transport_config: TransportConfig,
    drift_profile: DriftProfile,
    runtime: Arc<runtime::Runtime>,
    id: String,
) -> Result<(), EventError> {
    let mut queue = match QueueNum::new(
        transport_config,
        drift_profile,
        queue_event_loops.clone(),
        runtime,
    )
    .await
    {
        Ok(q) => {
            // set running to true
            *queue_running.write().unwrap() = true;
            q
        }
        Err(e) => {
            error!("Failed to initialize queue {}: {}", id, e);
            return Err(e);
        }
    };

    let mut running = true;
    while running {
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
                    },
                    Event::Start => {
                        debug!("Start event received for queue {}", id);
                        let mut events_running = events_running.write().unwrap();
                        *events_running = true;
                    },
                    Event::Stop => {
                        debug!("Stop event received for queue {}", id);
                        queue.flush().await?;
                        running = false;

                    }
                }
            }

            else => {
                debug!("Event channel closed for queue {}, shutting down", id);
                queue.flush().await?;
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
    transport_config: TransportConfig,
    queue_event_loops: HashMap<String, Arc<RwLock<EventLoops>>>,
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

    #[getter]
    /// Get the transport config for the ScouterQueue
    pub fn transport_config<'py>(
        &self,
        py: Python<'py>,
    ) -> Result<Bound<'py, PyAny>, PyEventError> {
        self.transport_config.to_py(py)
    }

    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }

    /// Triggers a global shutdown for all queues
    /// 1. This will call shutdown for all queues
    /// 2. The queues will be cleared from the hashmap
    /// 3. A loop will be run to ensure all background tasks have been shut down
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
        debug!("Starting ScouterQueue shutdown");

        // trigger shutdown for all queues
        for (id, queue) in &self.queues {
            let bound = queue.bind(py);
            debug!("Sending shutdown signal to queue: {}", id);
            bound
                .call_method0("shutdown")
                .map_err(PyEventError::ShutdownQueueError)?;
        }

        // Step 2: Wait for all background tasks to complete
        let max_wait_time = Duration::from_secs(10);
        let start_time = std::time::Instant::now();
        let check_interval = Duration::from_millis(50);

        while start_time.elapsed() < max_wait_time {
            let all_stopped = self.check_all_queues_stopped(py);

            if all_stopped {
                info!("All queues have stopped successfully");
                break;
            }

            std::thread::sleep(check_interval);
        }

        self.queues.clear();

        if !self.queues.is_empty() {
            return Err(PyEventError::PendingEventsError);
        }

        let mut queues_stopped = false;
        let max_retries = 100;
        let mut retries = 0;

        while !queues_stopped {
            let all_stopped = self.all_queues_stopped();
            if all_stopped {
                info!("All queues have stopped successfully");
                queues_stopped = true;
            } else {
                retries += 1;
                if retries > max_retries {
                    error!("Queues did not stop in time");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        debug!("All queues have been shutdown and cleared");

        Ok(())
    }

    fn check_all_queues_stopped(&self, py: Python) -> bool {
        for queue in self.queues.values() {
            let bound = queue.bind(py);

            // Check if the queue's running flag is false
            if let Ok(running) = bound.getattr("running") {
                if let Ok(is_running) = running.extract::<bool>() {
                    if is_running {
                        return false;
                    }
                }
            }
        }
        true
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
        let mut queue_states = HashMap::new();

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
            let queue_running = Arc::new(RwLock::new(false));

            // create startup channels to ensure queues are initialized before use
            let (bus, rx) = QueueBus::new();
            let clone_runtime = shared_runtime.clone();

            // spawn a new thread for each queue
            let id_clone = id.clone();

            // Just spawn the task without waiting for initialization
            shared_runtime.spawn(async move {
                match handle_queue_events(
                    rx,
                    cloned_config,
                    drift_profile,
                    clone_runtime,
                    queue_running.clone(),
                    id_clone,
                )
                .await
                {
                    Ok(running) => running,
                    Err(e) => {
                        error!("Queue initialization failed: {}", e);
                    }
                }
            });

            let queue = Py::new(py, bus)?;
            queues.insert(id.clone(), queue);
        }

        Ok(ScouterQueue {
            queues,
            // need to keep the runtime alive for the life of ScouterQueue
            _shared_runtime: shared_runtime,
            transport_config: config,
            queue_states,
        })
    }

    pub fn is_queue_running(&self, id: &str) -> bool {
        if let Some(queue) = self.queue_states.get(id) {
            return *queue.read().unwrap();
        }
        false
    }

    pub fn all_queues_stopped(&self) -> bool {
        self.queue_states.keys().all(|id| {
            let running = self.is_queue_running(id);
            info!("queue {} is running: {}", id, running);
            !running
        })
    }
}
