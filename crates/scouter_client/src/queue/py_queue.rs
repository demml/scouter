use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use pyo3::prelude::*;
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::psi::PsiDriftProfile;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::DriftType;
use scouter_types::Features;
use tracing::debug;

pub enum Queue {
    Spc(SpcQueue),
    Psi(PsiQueue),
}

impl Queue {
    pub fn new(
        drift_profile: &Bound<'_, PyAny>,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
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
            _ => Err(ScouterError::Error("Invalid drift type".to_string())),
        }
    }

    pub fn insert(&mut self, features: Features) -> Result<(), ScouterError> {
        match self {
            Queue::Spc(queue) => queue.insert(features),
            Queue::Psi(queue) => queue.insert(features),
        }
    }

    pub fn flush(&mut self) -> Result<(), ScouterError> {
        match self {
            Queue::Spc(queue) => queue.flush(),
            Queue::Psi(queue) => queue.flush(),
        }
    }
}

#[pyclass]
pub struct ScouterQueue {
    queue: Queue,
}

#[pymethods]
impl ScouterQueue {
    #[new]
    #[pyo3(signature = (drift_profile, config))]
    pub fn new(
        drift_profile: &Bound<'_, PyAny>,
        config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        Ok(ScouterQueue {
            queue: Queue::new(drift_profile, config)?,
        })
    }

    pub fn insert(&mut self, features: Features) -> PyResult<()> {
        self.queue
            .insert(features)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;
        debug!("Inserted features into queue");
        Ok(())
    }

    pub fn flush(&mut self) -> PyResult<()> {
        debug!("Flushing queue");
        match &mut self.queue {
            Queue::Spc(queue) => queue
                .flush()
                .map_err(|e| PyScouterError::new_err(e.to_string())),
            Queue::Psi(queue) => queue
                .flush()
                .map_err(|e| PyScouterError::new_err(e.to_string())),
        }
    }
}
