#![allow(clippy::useless_conversion)]
use super::types::TransportConfig;
use crate::queue::custom::CustomQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::queue::traits::queue::QueueMethods;
use pyo3::prelude::*;
use scouter_error::{EventError, PyScouterError, ScouterError};
use scouter_types::{DriftProfile, QueueEntity};
use scouter_types::{Features, Metrics};
use std::path::PathBuf;
use tracing::{error, info, instrument};

pub enum Queue {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
}

impl Queue {
    pub fn new(drift_profile: DriftProfile, config: TransportConfig) -> Result<Self, EventError> {
        match drift_profile {
            DriftProfile::Spc(spc_profile) => {
                let queue = SpcQueue::new(spc_profile, config)?;
                Ok(Queue::Spc(queue))
            }
            DriftProfile::Psi(psi_profile) => {
                let queue = PsiQueue::new(psi_profile, config)?;
                Ok(Queue::Psi(queue))
            }
            DriftProfile::Custom(custom_profile) => {
                let queue = CustomQueue::new(custom_profile, config)?;
                Ok(Queue::Custom(queue))
            }
        }
    }

    pub fn insert(&mut self, entity: QueueEntity) -> Result<(), EventError> {
        match entity {
            QueueEntity::Features(features) => self.insert_features(features),
            QueueEntity::Metrics(metrics) => self.insert_metrics(metrics),
        }
    }

    pub fn insert_features(&mut self, features: Features) -> Result<(), EventError> {
        match self {
            Queue::Psi(queue) => queue.insert(features),
            Queue::Spc(queue) => queue.insert(features),
            _ => Err(EventError::traced_insert_record_error("not supported")),
        }
    }

    pub fn insert_metrics(&mut self, metrics: Metrics) -> Result<(), EventError> {
        match self {
            Queue::Custom(queue) => queue.insert(metrics),
            _ => Err(EventError::traced_insert_record_error("not supported")),
        }
    }

    pub fn flush(&mut self) -> Result<(), EventError> {
        match self {
            Queue::Spc(queue) => queue.flush(),
            Queue::Psi(queue) => queue.flush(),
            Queue::Custom(queue) => queue.flush(),
        }
    }
}

pub struct ScouterQueue {
    queue: Queue,
}

impl ScouterQueue {
    /// Start the ScouterQueue (this is consuming method, meaning it will take ownership of the transport config)
    ///
    /// # Arguments
    /// * `transport_config` - The transport config to use
    pub fn new(transport_config: DriftTransportConfig) -> Result<Self, EventError> {
        info!("Starting ScouterQueue");

        Ok(ScouterQueue {
            queue: Queue::new(transport_config.drift_profile, transport_config.config)?,
        })
    }

    #[instrument(skip_all)]
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), EventError> {
        let entity = QueueEntity::from_py_entity(entity).map_err(|e| {
            error!(
                "Failed to extract entity from Python object: {:?}",
                e.to_string()
            );
            PyScouterError::new_err(e.to_string())
        })?;

        self.queue.insert(entity).map_err(|e| {
            error!("Failed to insert features into queue: {:?}", e.to_string());
            PyScouterError::new_err(e.to_string())
        })?;
        Ok(())
    }

    #[instrument(skip_all, name = "Flush", level = "debug")]
    pub fn flush(&mut self) -> Result<(), EventError> {
        match &mut self.queue {
            Queue::Spc(queue) => queue.flush().map_err(|e| {
                error!("Failed to flush queue: {:?}", e.to_string());
                PyScouterError::new_err(e.to_string())
            }),
            Queue::Psi(queue) => queue.flush().map_err(|e| {
                error!("Failed to flush queue: {:?}", e.to_string());
                PyScouterError::new_err(e.to_string())
            }),
            Queue::Custom(queue) => queue.flush().map_err(|e| {
                error!("Failed to flush queue: {:?}", e.to_string());
                PyScouterError::new_err(e.to_string())
            }),
        }
    }
}

#[pyclass]
pub struct DriftTransportConfig {
    pub drift_profile: DriftProfile,
    pub config: TransportConfig,

    #[pyo3(get)]
    pub id: String,
}

#[pymethods]
impl DriftTransportConfig {
    #[new]
    #[pyo3(signature = (id, transport_config, path))]
    pub fn from_path(
        id: String,
        transport_config: &Bound<'_, PyAny>,
        path: PathBuf,
    ) -> PyResult<Self> {
        let drift_profile = DriftProfile::from_profile_path(path)?;
        let config = TransportConfig::from_py_config(transport_config)?;

        Ok(DriftTransportConfig {
            id,
            drift_profile,
            config,
        })
    }

    #[getter]
    pub fn drift_profile<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.drift_profile.profile(py)
    }

    #[getter]
    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.config.to_py(py)?)
    }
}
