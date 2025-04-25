#![allow(clippy::useless_conversion)]
use super::types::TransportConfig;
use crate::queue::custom::CustomQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::QueueMethods;
use pyo3::prelude::*;
use scouter_error::{EventError, ScouterError};
use scouter_types::{DriftProfile, QueueEntity};
use scouter_types::{Features, Metrics};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, instrument};

pub enum QueueNum {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
}

impl QueueNum {
    pub fn new(drift_profile: DriftProfile, config: TransportConfig) -> Result<Self, EventError> {
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, config)?;
                Ok(QueueNum::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue = PsiQueue::new(psi_profile, config)?;
                Ok(QueueNum::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue = CustomQueue::new(custom_profile, config)?;
                Ok(QueueNum::Custom(queue))
            }
        }
    }

    /// Top-level insert method for the queue
    /// This method will take a QueueEntity and insert it into the appropriate queue
    /// If features, inserts using insert_features (spc, psi)
    /// If metrics, inserts using insert_metrics (custom)
    ///
    /// # Arguments
    /// * `entity` - The entity to insert into the queue
    pub fn insert(&mut self, entity: QueueEntity) -> Result<(), EventError> {
        match entity {
            QueueEntity::Features(features) => self.insert_features(features),
            QueueEntity::Metrics(metrics) => self.insert_metrics(metrics),
        }
    }

    /// Insert features into the queue. Currently only applies to PSI and SPC queues
    ///
    /// # Arguments
    /// * `features` - The features to insert into the queue
    ///
    ///
    pub fn insert_features(&mut self, features: Features) -> Result<(), EventError> {
        match self {
            QueueNum::Psi(queue) => queue.insert(features),
            QueueNum::Spc(queue) => queue.insert(features),
            _ => Err(EventError::traced_insert_record_error(
                "Queue not supported for feature entity",
            )),
        }
    }

    /// Insert metrics into the queue. Currently only applies to custom queues
    ///
    /// # Arguments
    /// * `metrics` - The metrics to insert into the queue
    ///
    pub fn insert_metrics(&mut self, metrics: Metrics) -> Result<(), EventError> {
        match self {
            QueueNum::Custom(queue) => queue.insert(metrics),
            _ => Err(EventError::traced_insert_record_error(
                "Queue not supported for metrics entity",
            )),
        }
    }

    /// Flush the queue. This will publish the records to the producer
    /// and shut down the background tasks
    pub fn flush(&mut self) -> Result<(), EventError> {
        match self {
            QueueNum::Spc(queue) => queue.flush(),
            QueueNum::Psi(queue) => queue.flush(),
            QueueNum::Custom(queue) => queue.flush(),
        }
    }
}

#[pyclass]
pub struct Queue {
    queue: QueueNum,
}

impl Queue {
    /// Start the ScouterQueue (this is consuming method, meaning it will take ownership of the transport config)
    ///
    /// # Arguments
    /// * `transport_config` - The transport config to use
    pub fn new(
        drift_profile: DriftProfile,
        transport_config: TransportConfig,
    ) -> Result<Self, EventError> {
        info!("Starting ScouterQueue");

        Ok(Queue {
            queue: QueueNum::new(drift_profile, transport_config)?,
        })
    }

    #[instrument(skip_all)]
    pub fn flush(&mut self) -> Result<(), ScouterError> {
        self.queue.flush()?;
        Ok(())
    }
}

#[pymethods]
impl Queue {
    #[instrument(skip_all)]
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), ScouterError> {
        let entity = QueueEntity::from_py_entity(entity)?;
        self.queue.insert(entity)?;
        Ok(())
    }
}

#[pyclass]
pub struct ScouterQueue {
    queues: HashMap<String, Py<Queue>>,
}

#[pymethods]
impl ScouterQueue {
    /// Create a new ScouterQueue from a map of aliases and paths
    /// This will create a new ScouterQueue for each path in the map
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
    ) -> Result<Self, ScouterError> {
        let mut queues = HashMap::new();
        let config = TransportConfig::from_py_config(transport_config)?;

        for (id, profile_path) in path {
            let cloned_config = config.clone();
            let drift_profile = DriftProfile::from_profile_path(profile_path)?;

            // create a python-bound queue for each path
            let queue = Py::new(
                py,
                Queue::new(drift_profile, cloned_config)
                    .map_err(|e| ScouterError::QueueCreateError(e.to_string()))?,
            )?;
            queues.insert(id, queue);
        }

        Ok(ScouterQueue { queues })
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
    ///
    pub fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &str,
    ) -> Result<&Bound<'py, Queue>, ScouterError> {
        match self.queues.get(key) {
            Some(queue) => Ok(queue.bind(py)),
            None => Err(ScouterError::MissingQueueError(key.to_string())),
        }
    }
}
