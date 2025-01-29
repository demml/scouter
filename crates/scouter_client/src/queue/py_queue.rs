use crate::queue::psi::PsiQueue;
use crate::queue::spc::SpcQueue;
use pyo3::prelude::*;
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::psi::PsiDriftProfile;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{DriftType, DriftProfile };
use scouter_types::Features;
use serde_json::Value;
use tracing::debug;
use tracing::info;

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
    #[pyo3(signature = (transport_config,))]
    pub fn new(
        transport_config: &Bound<'_, DriftTransportConfig>,
    ) -> Result<Self, ScouterError> {
        info!("Starting ScouterQueue");

        let profile = &transport_config.getattr("drift_profile")?;
        let config = &transport_config.getattr("config")?;

        Ok(ScouterQueue {
            queue: Queue::new(profile, config)?,
        })
    }

    pub fn insert(&mut self, features: Features) -> PyResult<()> {
        debug!("Inserting features into queue: {:?}", features);
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
    #[pyo3(signature = (id, config, drift_profile=None, drift_profile_path=None))]
    pub fn new(
        id: String,
        config: &Bound<'_, PyAny>,
        drift_profile: Option<&Bound<'_, PyAny>>,
        drift_profile_path: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {



        // if drift_profile_path and drift_profile are both missing, raise an error
        if drift_profile.is_none() && drift_profile_path.is_none() {
            return Err(PyScouterError::new_err(
                "Either drift_profile or drift_profile_path must be provided",
            ))
        }

        if drift_profile.is_none() && drift_profile_path.is_some() {
            // load drift_profile from path to serde_json::Value
            let profile =
                std::fs::read_to_string(drift_profile_path.unwrap().extract::<String>()?)?;
            let profile_value: Value = serde_json::from_str(&profile).unwrap();

            // get the drift_type from the drift_profile
            let drift_type = profile_value["config"]["drift_type"].as_str().unwrap();

            let drift_profile = DriftProfile::from_value(profile_value.clone(), drift_type)?;

            return Ok(DriftTransportConfig {
                id,
                drift_profile,
                config: config.clone().unbind(),
            });
            
            };
    

        let drift_type = drift_profile.unwrap().getattr("config")?.getattr("drift_type")?.extract::<DriftType>()?;
        Ok(DriftTransportConfig {
            id,
            drift_profile: DriftProfile::from_python(drift_type, drift_profile.unwrap())?,
            config: config.clone().unbind(),
        })
      
    }

    #[getter]
    pub fn drift_profile<'py>(&self, py:Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.drift_profile.profile(py)
    }

    #[getter]
    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<&Bound<'py, PyAny>> {
        Ok(self.config.bind(py))
    }
}
