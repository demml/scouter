#![allow(clippy::useless_conversion)]
use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_contracts::{
    DriftAlertRequest, DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
};
use scouter_error::{PyScouterError, ScouterError};
use scouter_events::producer::http::{HTTPClient, HTTPConfig, RequestType, Routes};
use scouter_types::{
    alert::Alert, custom::BinnedCustomMetrics, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures,
    DriftProfile, DriftType, ProfileFuncs,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::{debug, error};

pub const DOWNLOAD_CHUNK_SIZE: usize = 1024 * 1024 * 5;

#[pyclass]
pub struct ScouterClient {
    client: HTTPClient,
    rt: Arc<Runtime>,
}
#[pymethods]
impl ScouterClient {
    #[new]
    #[pyo3(signature = (config=None))]
    pub fn new(config: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let rt = Arc::new(Runtime::new()?);

        let config = config.map_or(Ok(HTTPConfig::default()), |unwrapped| {
            if unwrapped.is_instance_of::<HTTPConfig>() {
                unwrapped
                    .extract::<HTTPConfig>()
                    .map_err(|_| PyScouterError::new_err("Invalid config type"))
            } else {
                Err(PyScouterError::new_err("Invalid config type"))
            }
        })?;

        let client = rt
            .block_on(async { HTTPClient::new(config).await })
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        Ok(ScouterClient { client, rt })
    }

    /// Insert a profile into the scouter server
    ///
    /// # Arguments
    ///
    /// * `profile` - A profile object to insert
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the profile was inserted successfully
    #[pyo3(signature = (profile, set_active=false))]
    pub fn register_profile(
        &mut self,
        profile: &Bound<'_, PyAny>,
        set_active: bool,
    ) -> PyResult<bool> {
        let drift_type = profile
            .getattr("config")?
            .getattr("drift_type")?
            .extract::<DriftType>()?;

        let profile_str = profile
            .call_method0("model_dump_json")?
            .extract::<String>()?;

        let request = ProfileRequest {
            repository: profile
                .getattr("config")?
                .getattr("repository")?
                .extract::<String>()?,
            drift_type: drift_type.clone(),
            profile: profile_str,
        };

        let response = self
            .rt
            .block_on(async {
                self.client
                    .request(
                        Routes::Profile,
                        RequestType::Post,
                        Some(serde_json::to_value(&request).unwrap()),
                        None,
                        None,
                    )
                    .await
            })
            .map_err(|e: scouter_error::ScouterError| PyScouterError::new_err(e.to_string()))?;

        if response.status().is_success() {
            debug!("Profile inserted successfully");
            if set_active {
                let name = profile
                    .getattr("config")?
                    .getattr("name")?
                    .extract::<String>()?;

                let space = profile
                    .getattr("config")?
                    .getattr("space")?
                    .extract::<String>()?;

                let version = profile
                    .getattr("config")?
                    .getattr("version")?
                    .extract::<String>()?;

                let request = ProfileStatusRequest {
                    name,
                    space,
                    version,
                    active: true,
                    drift_type: Some(drift_type),
                };

                self.update_profile_status(request)
            } else {
                Ok(true)
            }
        } else {
            Err(PyScouterError::new_err(format!(
                "Failed to insert profile. Status: {:?}",
                response.status()
            )))
        }
    }

    /// Get binned drift data from the scouter server
    ///
    /// # Arguments
    ///
    /// * `drift_request` - A drift request object
    ///
    /// # Returns
    ///
    /// * A binned drift object
    pub fn get_binned_drift<'py>(
        &mut self,
        py: Python<'py>,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        match drift_request.drift_type {
            DriftType::Spc => self.rt.block_on(async {
                ScouterClient::get_spc_binned_drift(py, &mut self.client, drift_request).await
            }),
            DriftType::Psi => self.rt.block_on(async {
                ScouterClient::get_psi_binned_drift(py, &mut self.client, drift_request).await
            }),
            DriftType::Custom => self.rt.block_on(async {
                ScouterClient::get_custom_binned_drift(py, &mut self.client, drift_request).await
            }),
        }
    }

    pub fn update_profile_status(&mut self, request: ProfileStatusRequest) -> PyResult<bool> {
        debug!("Updating profile status: {:?}", request);
        let response = self
            .rt
            .block_on(async {
                self.client
                    .request(
                        Routes::ProfileStatus,
                        RequestType::Put,
                        Some(serde_json::to_value(request).unwrap()),
                        None,
                        None,
                    )
                    .await
            })
            .map_err(|e: scouter_error::ScouterError| PyScouterError::new_err(e.to_string()))?;

        Ok(response.status().is_success())
    }

    pub fn get_alerts(&mut self, request: DriftAlertRequest) -> PyResult<Vec<Alert>> {
        debug!("Getting alerts for: {:?}", request);

        let query_string =
            serde_qs::to_string(&request).map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let results = self
            .rt
            .block_on(async {
                let response = self
                    .client
                    .request(
                        Routes::Alerts,
                        RequestType::Get,
                        None,
                        Some(query_string),
                        None,
                    )
                    .await?;

                if response.status().is_client_error() || response.status().is_server_error() {
                    Err(PyScouterError::new_err(format!(
                        "Failed to get drift alerts. Status: {:?}",
                        response.status()
                    )))?
                } else {
                    let body: serde_json::Value = response.json().await.map_err(|e| {
                        error!(
                            "Failed to parse drift alerts {:?}. Error: {:?}",
                            &request, e
                        );
                        PyScouterError::new_err(e.to_string())
                    })?;

                    let data = body.get("data").unwrap().to_owned();

                    let results: Result<Vec<Alert>, ScouterError> = serde_json::from_value(data)
                        .map_err(|e| {
                            error!(
                                "Failed to parse drift alert response for {:?}. Error: {:?}",
                                &request, e
                            );
                            ScouterError::Error(e.to_string())
                        });

                    results
                }
            })
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        Ok(results)
    }

    #[pyo3(signature = (request, path))]
    pub fn download_profile(
        &mut self,
        request: GetProfileRequest,
        path: Option<PathBuf>,
    ) -> PyResult<String> {
        debug!("Downloading profile: {:?}", request);

        let filename = format!(
            "{}_{}_{}_{}.json",
            request.name, request.space, request.version, request.drift_type
        );

        let profile = self.get_drift_profile(request)?;

        ProfileFuncs::save_to_json(profile, path.clone(), &filename)?;

        Ok(path.map_or(filename, |p| p.to_string_lossy().to_string()))
    }
}

impl ScouterClient {
    async fn get_spc_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client
            .request(
                Routes::SpcDrift,
                RequestType::Get,
                None,
                Some(query_string),
                None,
            )
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().await.map_err(|e| {
            error!(
                "Failed to get drift data for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        let results: SpcDriftFeatures = serde_json::from_slice(&body).map_err(|e| {
            error!(
                "Failed to parse drift response for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        Ok(results.into_bound_py_any(py).unwrap())
    }
    async fn get_psi_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client
            .request(
                Routes::PsiDrift,
                RequestType::Get,
                None,
                Some(query_string),
                None,
            )
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().await.map_err(|e| {
            error!(
                "Failed to get drift data for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&body).map_err(|e| {
            error!(
                "Failed to parse drift response for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        Ok(results.into_bound_py_any(py).unwrap())
    }

    async fn get_custom_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client
            .request(
                Routes::CustomDrift,
                RequestType::Get,
                None,
                Some(query_string),
                None,
            )
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().await.map_err(|e| {
            error!(
                "Failed to get drift data for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        let results: BinnedCustomMetrics = serde_json::from_slice(&body).map_err(|e| {
            error!(
                "Failed to parse drift response for {:?}. Error: {:?}",
                &drift_request, e
            );
            ScouterError::Error(e.to_string())
        })?;

        Ok(results.into_bound_py_any(py).unwrap())
    }

    // TODO look into building an internal client responsible for making http calls to scouter-server, we can share this between the pyclient and internal rust calls
    pub fn get_drift_profile(
        &mut self,
        request: GetProfileRequest,
    ) -> Result<DriftProfile, ScouterError> {
        let query_string =
            serde_qs::to_string(&request).map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let results: Result<serde_json::Value, ScouterError> = self.rt.block_on(async {
            let response = self
                .client
                .request(
                    Routes::Profile,
                    RequestType::Get,
                    None,
                    Some(query_string),
                    None,
                )
                .await?;

            if response.status().is_client_error() || response.status().is_server_error() {
                Err(PyScouterError::new_err(format!(
                    "Failed to get profile. Status: {:?}",
                    response.status()
                )))?
            } else {
                let body = response.bytes().await.map_err(|e| {
                    error!("Failed to get profile for {:?}. Error: {:?}", &request, e);
                    ScouterError::Error(e.to_string())
                })?;

                let data: serde_json::Value = serde_json::from_slice(&body).map_err(|e| {
                    error!(
                        "Failed to parse profile response for {:?}. Error: {:?}",
                        &request, e
                    );
                    ScouterError::Error(e.to_string())
                })?;

                Ok(data)
            }
        });

        let profile = results
            .map_err(|e| ScouterError::Error(format!("Failed to download profile: {}", e)))?;

        let drift_profile = match request.drift_type {
            DriftType::Spc => DriftProfile::Spc(
                serde_json::from_value(profile).map_err(|_| ScouterError::DeSerializeError)?,
            ),
            DriftType::Psi => DriftProfile::Psi(
                serde_json::from_value(profile).map_err(|_| ScouterError::DeSerializeError)?,
            ),
            DriftType::Custom => DriftProfile::Custom(
                serde_json::from_value(profile).map_err(|_| ScouterError::DeSerializeError)?,
            ),
        };

        Ok(drift_profile)
    }
}
