#![allow(clippy::useless_conversion)]
use crate::error::{EventError, PyEventError};
use crate::queue::bus::Loop;
use crate::queue::bus::{Event, EventState, QueueBus};
use crate::queue::custom::CustomQueue;
use crate::queue::llm::LLMQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::wait_for_background_task;
use crate::queue::traits::queue::wait_for_event_task;
use crate::queue::traits::queue::BackgroundEvent;
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
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument};

fn create_event_state() -> (EventState, UnboundedReceiver<Event>) {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    // get background loop
    let background_loop = Arc::new(RwLock::new(Loop::new()));

    // get event loop
    let event_loop = Arc::new(RwLock::new(Loop::new()));

    let event_state = EventState {
        event_loop,
        background_loop,
        event_tx,
    };

    (event_state, event_rx)
}
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
        runtime: Arc<runtime::Runtime>,
        event_state: &mut EventState,
    ) -> Result<Self, EventError> {
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, transport_config).await?;
                Ok(QueueNum::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue =
                    PsiQueue::new(psi_profile, transport_config, runtime, event_state).await?;
                Ok(QueueNum::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue =
                    CustomQueue::new(custom_profile, transport_config, runtime, event_state)
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
async fn spawn_queue_event_handler(
    mut event_rx: UnboundedReceiver<Event>,
    transport_config: TransportConfig,
    drift_profile: DriftProfile,
    runtime: Arc<runtime::Runtime>,
    id: String,
    mut event_state: EventState,
    cancellation_token: CancellationToken,
) -> Result<(), EventError> {
    // This will create the specific queue based on the transport config and drift profile
    // Available queues:
    // - PSI - will also create a background task
    // - SPC
    // - Custom - will also create a background task
    // - LLM
    // event loops are used to monitor the background tasks of both custom and PSI queues
    let mut queue =
        match QueueNum::new(transport_config, drift_profile, runtime, &mut event_state).await {
            Ok(q) => q,
            Err(e) => {
                error!("Failed to initialize queue {}: {}", id, e);
                return Err(e);
            }
        };

    event_state.set_event_loop_running(true);

    loop {
        tokio::select! {
            Some(event) = event_rx.recv() => {
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

            _ = cancellation_token.cancelled() => {
                debug!("Stop signal received for queue {}", id);
                event_state.set_event_loop_running(false);
                break;
            }

            else => {
                debug!("Event channel closed for queue {}, shutting down", id);
                event_state.set_event_loop_running(false);
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
    pub queue_state: Arc<HashMap<String, EventState>>,
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
    pub fn shutdown(&mut self) -> Result<(), PyEventError> {
        debug!("Starting ScouterQueue shutdown");

        for (alias, event_state) in self.queue_state.iter() {
            debug!("Shutting down queue: {}", alias);
            // Flush first
            // shutdown the queue
            event_state.shutdown_tasks()?;
        }

        // clear the queues
        self.queues.clear();
        if !self.queues.is_empty() {
            return Err(PyEventError::PendingEventsError);
        }

        debug!("All queues have been shutdown and cleared");

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
        let mut queue_event_state = HashMap::new();

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
            let (mut event_state, event_rx) = create_event_state();

            // create startup channels to ensure queues are initialized before use
            let bus = QueueBus::new(event_state.clone());
            queue_event_state.insert(id.clone(), event_state.clone());
            let cancellation_token = CancellationToken::new();

            // queue args
            let clone_runtime = shared_runtime.clone();
            let id_clone = id.clone();
            let cloned_event_state = event_state.clone();

            // Spawn the task without waiting for initialization
            let handle = shared_runtime.spawn(async move {
                match spawn_queue_event_handler(
                    event_rx,
                    cloned_config,
                    drift_profile,
                    clone_runtime,
                    id_clone,
                    cloned_event_state,
                    cancellation_token.clone(),
                )
                .await
                {
                    Ok(running) => running,
                    Err(e) => {
                        error!("Queue initialization failed: {}", e);
                    }
                }
            });

            // add handle and stop tx to event loops for management
            event_state.add_event_handle(handle);
            event_state.add_event_stop_tx(event_stop_tx);

            std::thread::sleep(std::time::Duration::from_millis(100));

            // wait for background task and event task to signal startup
            wait_for_background_task(&event_state)?;
            wait_for_event_task(&event_state)?;

            let queue = Py::new(py, bus)?;
            queues.insert(id.clone(), queue);
        }

        Ok(ScouterQueue {
            queues,
            // need to keep the runtime alive for the life of ScouterQueue
            _shared_runtime: shared_runtime,
            transport_config: config,
            queue_event_state: Arc::new(queue_event_state),
        })
    }
}
