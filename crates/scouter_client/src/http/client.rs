use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_contracts::{DriftAlertRequest, DriftRequest, ProfileRequest, ProfileStatusRequest};
use scouter_error::{PyScouterError, ScouterError};
use scouter_events::producer::http::{HTTPClient, HTTPConfig, RequestType, Routes};
use scouter_types::{
    alert::Alert, custom::BinnedCustomMetrics, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures,
    DriftType,
};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::{debug, error};

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
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

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
            .unwrap();
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
            drift_type: drift_type.clone(),
            profile: profile_str,
        };

        let response = self
            .rt
            .block_on(async {
                let response = self
                    .client
                    .request_with_retry(
                        Routes::Profile,
                        RequestType::Post,
                        Some(serde_json::to_value(&request).unwrap()),
                        None,
                        None,
                    )
                    .await?;

                Ok(response)
            })
            .map_err(|e: scouter_error::ScouterError| PyScouterError::new_err(e.to_string()))?;

        if response.status().is_success() {
            debug!("Profile inserted successfully");
            if set_active {
                let name = profile
                    .getattr("config")?
                    .getattr("name")?
                    .extract::<String>()?;

                let repository = profile
                    .getattr("config")?
                    .getattr("repository")?
                    .extract::<String>()?;

                let version = profile
                    .getattr("config")?
                    .getattr("version")?
                    .extract::<String>()?;

                let request = ProfileStatusRequest {
                    name,
                    repository,
                    version,
                    active: true,
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
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        match drift_request.drift_type {
            DriftType::Spc => {
                let drift = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::SpcDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;

                        if response.status().is_client_error()
                            || response.status().is_server_error()
                        {
                            return Err(ScouterError::Error(format!(
                                "Failed to get drift data. Status: {:?}",
                                response.status()
                            )));
                        }

                        let body = response.bytes().await.unwrap();
                        let results: SpcDriftFeatures =
                            serde_json::from_slice(&body).map_err(|e| {
                                error!(
                                    "Failed to parse drift response for {:?}. Error: {:?}",
                                    &drift_request, e
                                );
                                ScouterError::Error(e.to_string())
                            })?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;

                Ok(drift.into_bound_py_any(py).unwrap())
            }
            DriftType::Psi => {
                let drift = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::PsiDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;

                        if response.status().is_client_error()
                            || response.status().is_server_error()
                        {
                            return Err(ScouterError::Error(format!(
                                "Failed to get drift data. Status: {:?}",
                                response.status()
                            )));
                        }

                        let body = response.bytes().await.unwrap();
                        let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&body)
                            .map_err(|e| {
                                error!(
                                    "Failed to parse drift response for {:?}. Error: {:?}",
                                    &drift_request, e
                                );
                                ScouterError::Error(e.to_string())
                            })?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;

                Ok(drift.into_bound_py_any(py).unwrap())
            }

            DriftType::Custom => {
                let drift = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::CustomDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;

                        if response.status().is_client_error()
                            || response.status().is_server_error()
                        {
                            return Err(ScouterError::Error(format!(
                                "Failed to get drift data. Status: {:?}",
                                response.status()
                            )));
                        }

                        let body = response.bytes().await.unwrap();
                        let results: BinnedCustomMetrics =
                            serde_json::from_slice(&body).map_err(|e| {
                                error!(
                                    "Failed to parse drift response for {:?}. Error: {:?}",
                                    &drift_request, e
                                );
                                ScouterError::Error(e.to_string())
                            })?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;

                Ok(drift.into_bound_py_any(py).unwrap())
            }
        }
    }

    pub fn update_profile_status(&mut self, request: ProfileStatusRequest) -> PyResult<bool> {
        debug!("Updating profile status: {:?}", request);
        let response = self
            .rt
            .block_on(async {
                let response = self
                    .client
                    .request_with_retry(
                        Routes::ProfileStatus,
                        RequestType::Put,
                        Some(serde_json::to_value(request).unwrap()),
                        None,
                        None,
                    )
                    .await?;

                Ok(response)
            })
            .map_err(|e: scouter_error::ScouterError| PyScouterError::new_err(e.to_string()))?;

        Ok(response.status().is_success())
    }

    pub fn get_alerts(&mut self, request: DriftAlertRequest) -> PyResult<Vec<Alert>> {
        debug!("Getting alerts for: {:?}", request);

        let query_string =
            serde_qs::to_string(&request).map_err(|e| PyScouterError::new_err(e.to_string()))?;

        self.rt
            .block_on(async {
                let response = self
                    .client
                    .request_with_retry(
                        Routes::Alerts,
                        RequestType::Get,
                        None,
                        Some(query_string),
                        None,
                    )
                    .await?;

                if response.status().is_client_error() || response.status().is_server_error() {
                    Err(ScouterError::Error(format!(
                        "Failed to get drift data. Status: {:?}",
                        response.status()
                    )))
                } else {
                    let body: serde_json::Value = response.json().await.unwrap();

                    let data = body.get("data").unwrap().to_owned();

                    let results: Vec<Alert> = serde_json::from_value(data).map_err(|e| {
                        error!(
                            "Failed to parse drift response for {:?}. Error: {:?}",
                            &request, e
                        );
                        ScouterError::Error(e.to_string())
                    })?;

                    Ok(results)
                }
            })
            .map_err(|e: scouter_error::ScouterError| PyScouterError::new_err(e.to_string()))
    }
}
