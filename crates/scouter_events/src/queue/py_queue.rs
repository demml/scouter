#![allow(clippy::useless_conversion)]
use crate::error::{EventError, PyEventError};
use crate::queue::bus::Task;
use crate::queue::bus::{Event, QueueBus, TaskState};
use crate::queue::custom::CustomQueue;
use crate::queue::genai::GenAIQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::wait_for_background_task;
use crate::queue::traits::queue::wait_for_event_task;
use crate::queue::traits::queue::QueueMethods;
use crate::queue::types::{QueueSettings, TransportConfig};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyListMethods};
use scouter_state::app_state;
use scouter_types::{DriftProfile, GenAIEvalRecord, QueueItem};
use scouter_types::{Features, Metrics};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument};

fn create_event_state(id: String) -> (TaskState, UnboundedReceiver<Event>) {
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    // get background loop
    let background_task = Arc::new(RwLock::new(Task::new()));

    // get event loop
    let event_task = Arc::new(RwLock::new(Task::new()));

    let event_state = TaskState {
        event_task,
        background_task,
        event_tx,
        id,
    };

    (event_state, event_rx)
}
pub enum QueueNum {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
    GenAI(GenAIQueue),
}
// need to add queue running lock to each and return it to the queue bus
impl QueueNum {
    pub async fn new(
        transport_config: TransportConfig,
        drift_profile: DriftProfile,
        task_state: &mut TaskState,
        queue_settings: Option<Arc<RwLock<QueueSettings>>>,
    ) -> Result<Self, EventError> {
        let identifier = drift_profile.identifier();
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, transport_config).await?;
                Ok(QueueNum::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue =
                    PsiQueue::new(psi_profile, transport_config, task_state, identifier).await?;
                Ok(QueueNum::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue =
                    CustomQueue::new(custom_profile, transport_config, task_state, identifier)
                        .await?;
                Ok(QueueNum::Custom(queue))
            }
            DriftProfile::GenAI(genai_profile) => {
                // settings cannot be None here
                let queue_settings = match queue_settings {
                    Some(s) => s,
                    None => {
                        return Err(EventError::MissingQueueSettingsError);
                    }
                };

                let queue = GenAIQueue::new(
                    genai_profile,
                    transport_config,
                    queue_settings,
                    task_state,
                    identifier,
                )
                .await?;
                Ok(QueueNum::GenAI(queue))
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
            QueueItem::GenAI(genai_record) => self.insert_genai_record(*genai_record).await,
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
    /// * `genai_record` - The LLM record to insert into the queue
    ///
    pub async fn insert_genai_record(
        &mut self,
        genai_record: GenAIEvalRecord,
    ) -> Result<(), EventError> {
        match self {
            QueueNum::GenAI(queue) => {
                if !queue.should_insert() {
                    debug!("Skipping LLM record insertion due to sampling rate");
                    return Ok(());
                }
                queue.insert(genai_record).await
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
            QueueNum::GenAI(queue) => queue.flush().await,
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all)]
async fn spawn_queue_event_handler(
    mut event_rx: UnboundedReceiver<Event>,
    transport_config: TransportConfig,
    drift_profile: DriftProfile,
    id: String,
    mut task_state: TaskState,
    cancellation_token: CancellationToken,
    queue_settings: Option<Arc<RwLock<QueueSettings>>>,
) -> Result<(), EventError> {
    // This will create the specific queue based on the transport config and drift profile
    // Available queues:
    // - PSI - will also create a background task
    // - SPC
    // - Custom - will also create a background task
    // - LLM
    // event loops are used to monitor the background tasks of both custom and PSI queues
    let mut queue = match QueueNum::new(
        transport_config,
        drift_profile,
        &mut task_state,
        queue_settings,
    )
    .await
    {
        Ok(q) => q,
        Err(e) => {
            error!("Failed to initialize queue {}: {}", id, e);
            return Err(e);
        }
    };

    task_state.set_event_running(true);
    task_state.notify_event_started();
    debug!("Event loop for queue {} set to running", id);
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
                    Event::Flush => {
                        debug!("Flush event received for queue {}", id);
                        match queue.flush().await {
                            Ok(_) => {
                                debug!("Successfully flushed queue {}", id);
                            }
                            Err(e) => {
                                error!("Error flushing queue {}: {}", id, e);
                            }
                        }
                    }
                }
            }

            _ = cancellation_token.cancelled() => {
                debug!("Stop signal received for queue {}", id);
                match queue.flush().await {
                    Ok(_) => {
                        debug!("Successfully flushed queue {}", id);
                    }
                    Err(e) => {
                        error!("Error flushing queue {}: {}", id, e);
                    }
                }
                task_state.set_event_running(false);
                break;
            }

            else => {
                debug!("Event channel closed for queue {}, shutting down", id);
                match queue.flush().await {
                    Ok(_) => {
                        debug!("Successfully flushed queue {}", id);
                    }
                    Err(e) => {
                        error!("Error flushing queue {}: {}", id, e);
                    }
                }
                task_state.set_event_running(false);
                break;
            }
        }
    }
    Ok(())
}

// need to add version here
#[pyclass]
pub struct ScouterQueue {
    queues: HashMap<String, Py<QueueBus>>,
    transport_config: TransportConfig,
    // this is used to update settings for a particular queue
    // Key is the alias of the queue
    settings: HashMap<String, Arc<RwLock<QueueSettings>>>,
    pub queue_state: Arc<HashMap<String, TaskState>>,
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
    /// * *wait_for_startup* - Whether to wait for each queue to signal startup before returning
    /// # Returns
    /// * `ScouterQueue` - A new ScouterQueue
    #[staticmethod]
    #[pyo3(signature = (path, transport_config, wait_for_startup=false))]
    pub fn from_path(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
        wait_for_startup: bool,
    ) -> Result<Self, PyEventError> {
        ScouterQueue::from_path_rs(py, path, transport_config, wait_for_startup)
    }

    /// Create a new ScouterQueue from a drift profile.
    /// This is used for programmatic creation of queues without needing to read from a path.
    /// This is useful for testing and for dynamic queue creation.
    /// # Arguments
    /// * `profile` - A dict, list, or single drift profile object
    /// * `transport_config` - The transport config to use
    /// * *wait_for_startup* - Whether to wait for each queue to signal startup before returning
    /// # Returns
    /// * `ScouterQueue` - A new ScouterQueue
    #[staticmethod]
    #[pyo3(signature = (profile, transport_config, wait_for_startup=false))]
    pub fn from_profile(
        py: Python,
        profile: &Bound<'_, PyAny>,
        transport_config: &Bound<'_, PyAny>,
        wait_for_startup: bool,
    ) -> Result<Self, PyEventError> {
        debug!("Creating ScouterQueue from profile");
        let profiles = extract_drift_profiles(profile)?;
        ScouterQueue::from_profile_rs(py, profiles, transport_config, wait_for_startup)
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

    /// Update the sample ratio for all queues
    pub fn _set_sample_ratio(&self, sample_ratio: f64) -> Result<(), PyEventError> {
        for (alias, settings) in self.settings.iter() {
            debug!(
                "Updating sample ratio for queue {} to {}",
                alias, sample_ratio
            );
            let mut settings_write = settings.write().unwrap();
            settings_write.update_sample_ratio(sample_ratio);
        }
        Ok(())
    }
}

impl ScouterQueue {
    #[instrument(skip_all)]
    fn initialize_queue(
        py: Python,
        id: String,
        drift_profile: DriftProfile,
        config: TransportConfig,
        queue_state: &mut HashMap<String, TaskState>,
        queue_settings: &mut HashMap<String, Arc<RwLock<QueueSettings>>>,
        wait_for_startup: bool,
    ) -> Result<Py<QueueBus>, PyEventError> {
        let settings = if let DriftProfile::GenAI(genai_profile) = &drift_profile {
            let settings = Arc::new(RwLock::new(QueueSettings::new(
                id.clone(),
                genai_profile.config.sample_ratio,
            )));
            queue_settings.insert(id.clone(), settings.clone());
            Some(settings)
        } else {
            None
        };

        let (mut event_state, event_rx) = create_event_state(id.clone());
        let bus = QueueBus::new(
            event_state.clone(),
            drift_profile.identifier(),
            drift_profile.uid().to_string(),
        );
        queue_state.insert(id.clone(), event_state.clone());

        let cancellation_token = CancellationToken::new();
        let cloned_cancellation_token = cancellation_token.clone();

        let runtime_handle = app_state().handle();
        let id_clone = id.clone();
        let cloned_event_state = event_state.clone();

        let handle = runtime_handle.spawn(async move {
            match spawn_queue_event_handler(
                event_rx,
                config,
                drift_profile,
                id_clone,
                cloned_event_state,
                cloned_cancellation_token,
                settings,
            )
            .await
            {
                Ok(running) => running,
                Err(e) => {
                    error!("Queue initialization failed: {}", e);
                }
            }
        });

        event_state.add_event_abort_handle(handle);
        event_state.add_event_cancellation_token(cancellation_token);

        if wait_for_startup {
            debug!("Waiting for queue {} to signal startup", id);
            runtime_handle.block_on(async {
                wait_for_background_task(&event_state).await?;
                wait_for_event_task(&event_state).await
            })?;
            debug!("Queue {} has signaled startup", id);
        }

        Ok(Py::new(py, bus)?)
    }

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
    #[instrument(skip_all)]
    pub fn from_path_rs(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
        wait_for_startup: bool,
    ) -> Result<Self, PyEventError> {
        debug!("Creating ScouterQueue from path");
        let mut queues = HashMap::new();
        let mut queue_state = HashMap::new();
        let mut queue_settings = HashMap::new();

        if transport_config.is_none() {
            return Err(PyEventError::MissingTransportConfig);
        }

        let config = TransportConfig::from_py_config(transport_config)?;

        for (id, profile_path) in path {
            let drift_profile = DriftProfile::from_profile_path(profile_path)?;
            let queue = Self::initialize_queue(
                py,
                id.clone(),
                drift_profile,
                config.clone(),
                &mut queue_state,
                &mut queue_settings,
                wait_for_startup,
            )?;
            queues.insert(id, queue);
        }

        Ok(ScouterQueue {
            queues,
            transport_config: config,
            queue_state: Arc::new(queue_state),
            settings: queue_settings,
        })
    }

    #[instrument(skip_all)]
    pub fn from_profile_rs(
        py: Python,
        profiles: HashMap<String, DriftProfile>,
        transport_config: &Bound<'_, PyAny>,
        wait_for_startup: bool,
    ) -> Result<Self, PyEventError> {
        debug!("Creating ScouterQueue from profiles");
        let mut queues = HashMap::new();
        let mut queue_state = HashMap::new();
        let mut queue_settings = HashMap::new();

        if transport_config.is_none() {
            return Err(PyEventError::MissingTransportConfig);
        }

        let config = TransportConfig::from_py_config(transport_config)?;

        for (id, drift_profile) in profiles {
            let queue = Self::initialize_queue(
                py,
                id.clone(),
                drift_profile,
                config.clone(),
                &mut queue_state,
                &mut queue_settings,
                wait_for_startup,
            )?;
            queues.insert(id, queue);
        }

        Ok(ScouterQueue {
            queues,
            transport_config: config,
            queue_state: Arc::new(queue_state),
            settings: queue_settings,
        })
    }
}

/// Extract drift profiles from Python objects into a HashMap
/// Supports three input formats:
/// 1. Dict[str, DriftProfile] - Map of aliases to profiles
/// 2. List[DriftProfile] - List of profiles (each must have alias attribute)
/// 3. Single DriftProfile - Single profile with alias attribute
fn extract_drift_profiles(
    py_profiles: &Bound<'_, PyAny>,
) -> Result<HashMap<String, DriftProfile>, PyEventError> {
    if py_profiles.is_instance_of::<PyDict>() {
        let py_dict = py_profiles.cast::<PyDict>()?;
        let mut profiles = HashMap::new();

        for (alias, profile) in py_dict.iter() {
            let alias = alias.extract::<String>()?;
            let drift_profile = DriftProfile::from_python(&profile)?;
            profiles.insert(alias, drift_profile);
        }

        Ok(profiles)
    } else if py_profiles.is_instance_of::<PyList>() {
        let py_list = py_profiles.cast::<PyList>()?;
        let mut profiles = HashMap::new();

        for profile in py_list.iter() {
            let alias = profile
                .getattr("alias")?
                .extract::<Option<String>>()?
                .ok_or(PyEventError::DriftProfileAliasMustBeSet)?;

            let drift_profile = DriftProfile::from_python(&profile)?;
            profiles.insert(alias, drift_profile);
        }

        Ok(profiles)
    } else if py_profiles.hasattr("alias")? {
        let alias = py_profiles
            .getattr("alias")?
            .extract::<Option<String>>()?
            .ok_or(PyEventError::DriftProfileAliasMustBeSet)?;

        let drift_profile = DriftProfile::from_python(py_profiles)?;
        let mut profiles = HashMap::new();
        profiles.insert(alias, drift_profile);

        Ok(profiles)
    } else {
        Err(PyEventError::InvalidDriftProfileFormat)
    }
}
