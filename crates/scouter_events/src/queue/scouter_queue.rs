#![allow(clippy::useless_conversion)]
use crate::queue::custom::CustomQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use pyo3::prelude::*;
use scouter_error::{EventError, PyScouterError, ScouterError};
use scouter_types::custom::CustomDriftProfile;
use scouter_types::psi::PsiDriftProfile;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{DriftProfile, DriftType};
use scouter_types::{Features, Metrics};
use serde_json::Value;
use std::path::PathBuf;
use tracing::{error, info, instrument};

use super::types::TransportConfig;

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

    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), ScouterError> {
        match self {
            Queue::Spc(queue) => {
                let features = entity.extract::<Features>()?;
                queue.insert(features)
            }
            Queue::Psi(queue) => {
                let features = entity.extract::<Features>()?;
                queue.insert(features)
            }
            Queue::Custom(queue) => {
                let metrics = entity.extract::<Metrics>()?;
                queue.insert(metrics)
            }
        }
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
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
    pub fn new(transport_config: &DriftTransportConfig) -> Result<Self, EventError> {
        info!("Starting ScouterQueue");

        let profile = transport_config.drift_profile;
        let config = transport_config.config;

        Ok(ScouterQueue {
            queue: Queue::new(profile, config)?,
        })
    }

    #[instrument(skip_all, name = "Insert", level = "debug", err)]
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), EventError> {
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
