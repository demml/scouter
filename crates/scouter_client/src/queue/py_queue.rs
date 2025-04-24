#![allow(clippy::useless_conversion)]
use super::custom::CustomQueue;
use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use crate::ScouterClient;
use pyo3::prelude::*;
use scouter_contracts::GetProfileRequest;
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::custom::CustomDriftProfile;
use scouter_types::psi::PsiDriftProfile;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{DriftProfile, DriftType};
use scouter_types::{Features, Metrics};
use serde_json::Value;
use std::path::PathBuf;
use tracing::{error, info, instrument};

pub enum Queue {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
}

impl Queue {
    pub fn new(drift_profile: &DriftProfile, config: &C) -> Result<Self, ScouterError> {
        let drift_type = drift_profile
            .getattr("config")?
            .getattr("drift_type")?
            .extract::<DriftType>()?;

        match drift_type {
            DriftType::Spc => {
                let drift_profile = drift_profile.extract::<SpcDriftProfile>()?;
                Ok(Queue::Spc(SpcQueue::new(drift_profile, config)?))
            }
            DriftType::Psi => {
                let drift_profile = drift_profile.extract::<PsiDriftProfile>()?;
                Ok(Queue::Psi(PsiQueue::new(drift_profile, config)?))
            }
            DriftType::Custom => {
                let drift_profile = drift_profile.extract::<CustomDriftProfile>()?;
                Ok(Queue::Custom(CustomQueue::new(drift_profile, config)?))
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
    pub fn new(transport_config: &DriftTransportConfig) -> Result<Self, ScouterError> {
        info!("Starting ScouterQueue");

        let profile = &transport_config.drift_profile;
        let config = &transport_config.config;

        Ok(ScouterQueue {
            queue: Queue::new(profile, config)?,
        })
    }

    #[instrument(skip(self, entity), name = "Insert", level = "debug")]
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> PyResult<()> {
        self.queue.insert(entity).map_err(|e| {
            error!("Failed to insert features into queue: {:?}", e.to_string());
            PyScouterError::new_err(e.to_string())
        })?;
        Ok(())
    }

    #[instrument(skip(self), name = "Flush", level = "debug")]
    pub fn flush(&mut self) -> PyResult<()> {
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
    pub config: PyObject,
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
        let profile = std::fs::read_to_string(path)?;
        let profile_value: Value = serde_json::from_str(&profile).unwrap();
        let drift_profile = DriftProfile::from_value(profile_value)?;

        Ok(DriftTransportConfig {
            id,
            drift_profile,
            config: config.clone().unbind(),
        })
    }

    #[getter]
    pub fn drift_profile<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.drift_profile.profile(py)
    }

    #[getter]
    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<&Bound<'py, PyAny>> {
        Ok(self.config.bind(py))
    }
}
