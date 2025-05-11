#![allow(clippy::useless_conversion)]
use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_error::{PyScouterError, ScouterError};
use scouter_settings::http::HTTPConfig;
use scouter_types::contracts::{
    DriftAlertRequest, DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
};
use scouter_types::http::{RequestType, Routes};

use crate::http::HTTPClient;
use scouter_types::{
    alert::Alert, custom::BinnedCustomMetrics, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures,
    DriftProfile, DriftType, ProfileFuncs,
};
use std::path::PathBuf;
use tracing::{debug, error};

pub const DOWNLOAD_CHUNK_SIZE: usize = 1024 * 1024 * 5;

#[derive(Debug, Clone)]
pub struct ScouterClient {
    client: HTTPClient,
}

impl ScouterClient {
    pub fn new(config: Option<HTTPConfig>) -> Result<Self, ScouterError> {
        let client = HTTPClient::new(config.unwrap_or_default())
            .map_err(|e| ScouterError::Error(format!("Failed to create HTTP client: {}", e)))?;

        Ok(ScouterClient { client })
    }

    /// Insert a profile into the scouter server
    pub fn insert_profile(&mut self, request: &ProfileRequest) -> Result<bool, ScouterError> {
        let response = self
            .client
            .request(
                Routes::Profile,
                RequestType::Post,
                Some(serde_json::to_value(request).unwrap()),
                None,
                None,
            )
            .map_err(|e| ScouterError::Error(format!("Failed to insert profile: {}", e)))?;

        if response.status().is_success() {
            Ok(true)
        } else {
            Err(ScouterError::Error(format!(
                "Failed to insert profile. Status: {:?}",
                response.status()
            )))
        }
    }

    pub fn update_profile_status(
        &mut self,
        request: &ProfileStatusRequest,
    ) -> Result<bool, ScouterError> {
        let response = self
            .client
            .request(
                Routes::ProfileStatus,
                RequestType::Put,
                Some(serde_json::to_value(request).unwrap()),
                None,
                None,
            )
            .map_err(|e| ScouterError::Error(format!("Failed to update profile status: {}", e)))?;

        if response.status().is_success() {
            Ok(true)
        } else {
            Err(ScouterError::Error(format!(
                "Failed to update profile status. Status: {:?}",
                response.status()
            )))
        }
    }

    pub fn get_alerts(&mut self, request: &DriftAlertRequest) -> Result<Vec<Alert>, ScouterError> {
        debug!("Getting alerts for: {:?}", request);

        let query_string = serde_qs::to_string(request)
            .map_err(|e| ScouterError::Error(format!("Failed to serialize request: {}", e)))?;

        let response = self.client.request(
            Routes::Alerts,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        // Check response status
        if !response.status().is_success() {
            return Err(ScouterError::Error(format!(
                "Failed to get drift alerts. Status: {:?}",
                response.status()
            )));
        }

        // Parse response body
        let body: serde_json::Value = response.json().map_err(|e| {
            error!(
                "Failed to parse drift alerts {:?}. Error: {:?}",
                &request, e
            );
            ScouterError::Error(format!("Failed to parse response: {}", e))
        })?;

        // Extract alerts from response
        let alerts = body
            .get("alerts")
            .map(|alerts| {
                serde_json::from_value::<Vec<Alert>>(alerts.clone()).map_err(|e| {
                    error!(
                        "Failed to parse drift alerts {:?}. Error: {:?}",
                        &request, e
                    );
                    ScouterError::Error(format!("Failed to parse alerts: {}", e))
                })
            })
            .unwrap_or_else(|| {
                error!("No alerts found in response");
                Ok(Vec::new())
            })?;

        Ok(alerts)
    }

    pub fn get_drift_profile(
        &mut self,
        request: GetProfileRequest,
    ) -> Result<DriftProfile, ScouterError> {
        let query_string = serde_qs::to_string(&request)
            .map_err(|e| ScouterError::Error(format!("Failed to serialize request: {}", e)))?;

        let response = self.client.request(
            Routes::Profile,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        // Early return for error status codes
        if !response.status().is_success() {
            error!("Failed to get profile. Status: {:?}", response.status());
            return Err(ScouterError::Error(format!(
                "Server returned error status: {}",
                response.status()
            )));
        }

        // Get response body
        let body = response.bytes().map_err(|e| {
            error!("Failed to get profile for {:?}. Error: {:?}", &request, e);
            ScouterError::Error(format!("Failed to read response body: {}", e))
        })?;

        // Parse JSON response
        let profile: DriftProfile = serde_json::from_slice(&body).map_err(|e| {
            error!(
                "Failed to parse profile response for {:?}. Error: {:?}",
                &request, e
            );
            ScouterError::Error(format!("Failed to parse JSON response: {}", e))
        })?;

        Ok(profile)
    }

    /// Check if the scouter server is healthy
    pub fn check_service_health(&mut self) -> Result<bool, ScouterError> {
        let response = self
            .client
            .request(Routes::Healthcheck, RequestType::Get, None, None, None)
            .inspect_err(|e| {
                error!("Failed to check scouter health {}", e);
            })?;

        if response.status() == 200 {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[pyclass(name = "ScouterClient")]
pub struct PyScouterClient {
    client: ScouterClient,
}
#[pymethods]
impl PyScouterClient {
    #[new]
    #[pyo3(signature = (config=None))]
    pub fn new(config: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let config = config.map_or(Ok(HTTPConfig::default()), |unwrapped| {
            if unwrapped.is_instance_of::<HTTPConfig>() {
                unwrapped
                    .extract::<HTTPConfig>()
                    .map_err(|_| PyScouterError::new_err("Invalid config type"))
            } else {
                Err(PyScouterError::new_err("Invalid config type"))
            }
        })?;

        let client = ScouterClient::new(Some(config.clone())).map_err(|e| {
            PyScouterError::new_err(format!("Failed to create ScouterClient: {}", e))
        })?;

        Ok(PyScouterClient { client })
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
    #[pyo3(signature = (profile, set_active=false, deactivate_others=false))]
    pub fn register_profile(
        &mut self,
        profile: &Bound<'_, PyAny>,
        set_active: bool,
        deactivate_others: bool,
    ) -> PyResult<bool> {
        let request = profile
            .call_method0("create_profile_request")?
            .extract::<ProfileRequest>()?;

        self.client
            .insert_profile(&request)
            .map_err(|e| PyScouterError::new_err(format!("Failed to insert profile: {}", e)))?;

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

            let drift_type = profile
                .getattr("config")?
                .getattr("drift_type")?
                .extract::<DriftType>()?;

            let request = ProfileStatusRequest {
                name,
                space,
                version,
                active: true,
                drift_type: Some(drift_type),
                deactivate_others,
            };

            self.client.update_profile_status(&request).map_err(|e| {
                PyScouterError::new_err(format!("Failed to update profile status: {}", e))
            })?;
        }

        Ok(true)
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
            DriftType::Spc => {
                PyScouterClient::get_spc_binned_drift(py, &mut self.client.client, drift_request)
            }
            DriftType::Psi => {
                PyScouterClient::get_psi_binned_drift(py, &mut self.client.client, drift_request)
            }
            DriftType::Custom => {
                PyScouterClient::get_custom_binned_drift(py, &mut self.client.client, drift_request)
            }
        }
    }

    pub fn get_alerts(&mut self, request: DriftAlertRequest) -> PyResult<Vec<Alert>> {
        debug!("Getting alerts for: {:?}", request);

        let alerts = self
            .client
            .get_alerts(&request)
            .map_err(|e| PyScouterError::new_err(format!("Failed to get alerts: {}", e)))?;

        Ok(alerts)
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

        let profile = self.client.get_drift_profile(request)?;

        ProfileFuncs::save_to_json(profile, path.clone(), &filename)?;

        Ok(path.map_or(filename, |p| p.to_string_lossy().to_string()))
    }
}

impl PyScouterClient {
    fn get_spc_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client.request(
            Routes::SpcDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().map_err(|e| {
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
    fn get_psi_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client.request(
            Routes::PsiDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().map_err(|e| {
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

    fn get_custom_binned_drift<'py>(
        py: Python<'py>,
        client: &mut HTTPClient,
        drift_request: DriftRequest,
    ) -> PyResult<Bound<'py, PyAny>> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        let response = client.request(
            Routes::CustomDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(PyScouterError::new_err(format!(
                "Failed to get drift data. Status: {:?}",
                response.status()
            )));
        }

        let body = response.bytes().map_err(|e| {
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
}
