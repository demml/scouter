use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_contracts::{DriftRequest, ProfileRequest};
use scouter_error::{PyScouterError, ScouterError};
use scouter_events::producer::http::{HTTPClient, HTTPConfig, RequestType, Routes};
use scouter_types::DriftType;
use scouter_types::{
    custom::BinnedCustomMetrics, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures,
};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::error;

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
    pub fn register_profile(&mut self, profile: &Bound<'_, PyAny>) -> PyResult<()> {
        let drift_type = profile
            .getattr("config")?
            .getattr("drift_type")?
            .extract::<DriftType>()?;
        let profile = profile
            .call_method0("model_dump_json")?
            .extract::<String>()?;

        let request = ProfileRequest {
            drift_type,
            profile,
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
            Ok(())
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
}
