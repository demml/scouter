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
use std::sync::Arc;
use tracing::{error, info, instrument};

pub enum Queue {
    Spc(SpcQueue),
    Psi(PsiQueue),
    Custom(CustomQueue),
}

impl Queue {
    pub async fn new(
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
                Ok(Queue::Spc(SpcQueue::new(drift_profile, config).await?))
            }
            DriftType::Psi => {
                let drift_profile = drift_profile.extract::<PsiDriftProfile>()?;
                Ok(Queue::Psi(PsiQueue::new(drift_profile, config).await?))
            }
            DriftType::Custom => {
                let drift_profile = drift_profile.extract::<CustomDriftProfile>()?;
                Ok(Queue::Custom(
                    CustomQueue::new(drift_profile, config).await?,
                ))
            }
        }
    }

    pub async fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), ScouterError> {
        match self {
            Queue::Spc(queue) => {
                let features = entity.extract::<Features>()?;
                queue.insert(features).await
            }
            Queue::Psi(queue) => {
                let features = entity.extract::<Features>()?;
                queue.insert(features).await
            }
            Queue::Custom(queue) => {
                let metrics = entity.extract::<Metrics>()?;
                queue.insert(metrics).await
            }
        }
    }

    pub async fn flush(&mut self) -> Result<(), ScouterError> {
        match self {
            Queue::Spc(queue) => queue.flush().await,
            Queue::Psi(queue) => queue.flush().await,
            Queue::Custom(queue) => queue.flush().await,
        }
    }
}

#[pyclass]
pub struct ScouterQueue {
    queue: Queue,
    rt: Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl ScouterQueue {
    #[new]
    #[pyo3(signature = (transport_config,))]
    pub fn new(transport_config: &Bound<'_, DriftTransportConfig>) -> PyResult<ScouterQueue> {
        info!("Starting ScouterQueue");

        let rt = Arc::new(tokio::runtime::Runtime::new().map_err(|e| {
            error!("Failed to create tokio runtime: {:?}", e.to_string());
            PyScouterError::new_err(e.to_string())
        })?);

        let profile = &transport_config.getattr("drift_profile").map_err(|e| {
            error!("Failed to get drift_profile: {:?}", e.to_string());
            PyScouterError::new_err(e.to_string())
        })?;
        let config = &transport_config.getattr("config")?;

        let queue = rt.block_on(async { Queue::new(profile, config).await })?;

        Ok(ScouterQueue { queue, rt })
    }

    #[allow(clippy::useless_conversion)]
    #[instrument(skip(self, entity), name = "Insert", level = "debug")]
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> PyResult<()> {
        self.rt
            .block_on(async { self.queue.insert(entity).await })
            .map_err(|e| {
                error!("Failed to insert features into queue: {:?}", e.to_string());
                PyScouterError::new_err(e.to_string())
            })?;

        Ok(())
    }

    #[allow(clippy::useless_conversion)]
    #[instrument(skip_all, name = "Flush", level = "debug")]
    pub fn flush(&mut self) -> PyResult<()> {
        self.rt
            .block_on(async {
                match &mut self.queue {
                    Queue::Spc(queue) => queue.flush().await.map_err(|e| {
                        error!("Failed to flush queue: {:?}", e.to_string());
                        PyScouterError::new_err(e.to_string())
                    }),
                    Queue::Psi(queue) => queue.flush().await.map_err(|e| {
                        error!("Failed to flush queue: {:?}", e.to_string());
                        PyScouterError::new_err(e.to_string())
                    }),
                    Queue::Custom(queue) => queue.flush().await.map_err(|e| {
                        error!("Failed to flush queue: {:?}", e.to_string());
                        PyScouterError::new_err(e.to_string())
                    }),
                }
            })
            .map_err(|e| {
                error!("Failed to flush queue: {:?}", e.to_string());
                PyScouterError::new_err(e.to_string())
            })?;

        Ok(())
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
    #[pyo3(signature = (id, config, drift_profile_path=None, drift_profile_request=None, scouter_server_config=None))]
    pub fn new(
        id: String,
        config: &Bound<'_, PyAny>,
        drift_profile_path: Option<PathBuf>,
        drift_profile_request: Option<GetProfileRequest>,
        scouter_server_config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // if drift_profile_path and drift_profile_request are both missing, raise an error
        if drift_profile_path.is_none() && drift_profile_request.is_none() {
            return Err(PyScouterError::new_err(
                "Either drift_profile_path or drift_profile_request must be provided",
            ));
        }

        if drift_profile_path.is_some() {
            // load drift_profile from path to serde_json::Value
            let profile = std::fs::read_to_string(drift_profile_path.unwrap())?;
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

        let mut client = ScouterClient::new(scouter_server_config)?;

        let drift_profile = client.get_drift_profile(drift_profile_request.unwrap())?;

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
