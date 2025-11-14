#![allow(clippy::useless_conversion)]
use crate::error::ClientError;
use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_settings::http::HTTPConfig;
use scouter_types::contracts::{
    DriftAlertRequest, DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
};
use scouter_types::http::{RequestType, Routes};
use scouter_types::sql::TraceFilters;
use scouter_types::{
    RegisteredProfileResponse, TracePaginationResponse, TraceRequest, TraceSpansResponse,
};

use crate::http::HTTPClient;
use scouter_types::{
    alert::Alert, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures, BinnedMetrics, DriftProfile,
    DriftType, PyHelperFuncs,
};
use std::path::PathBuf;
use tracing::{debug, error};

pub const DOWNLOAD_CHUNK_SIZE: usize = 1024 * 1024 * 5;

#[derive(Debug, Clone)]
pub struct ScouterClient {
    client: HTTPClient,
}

impl ScouterClient {
    pub fn new(config: Option<HTTPConfig>) -> Result<Self, ClientError> {
        let client = HTTPClient::new(config.unwrap_or_default())?;

        Ok(ScouterClient { client })
    }

    /// Insert a profile into the scouter server
    pub fn insert_profile(
        &self,
        request: &ProfileRequest,
    ) -> Result<RegisteredProfileResponse, ClientError> {
        let response = self.client.request(
            Routes::Profile,
            RequestType::Post,
            Some(serde_json::to_value(request).unwrap()),
            None,
            None,
        )?;

        if response.status().is_success() {
            let body = response.bytes()?;
            let profile_response: RegisteredProfileResponse = serde_json::from_slice(&body)?;

            debug!("Profile inserted successfully: {:?}", profile_response);
            Ok(profile_response)
        } else {
            Err(ClientError::InsertProfileError)
        }
    }

    pub fn update_profile_status(
        &self,
        request: &ProfileStatusRequest,
    ) -> Result<bool, ClientError> {
        let response = self.client.request(
            Routes::ProfileStatus,
            RequestType::Put,
            Some(serde_json::to_value(request).unwrap()),
            None,
            None,
        )?;

        if response.status().is_success() {
            Ok(true)
        } else {
            Err(ClientError::UpdateProfileError)
        }
    }

    pub fn get_alerts(&self, request: &DriftAlertRequest) -> Result<Vec<Alert>, ClientError> {
        debug!("Getting alerts for: {:?}", request);

        let query_string = serde_qs::to_string(request)?;

        let response = self.client.request(
            Routes::Alerts,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        // Check response status
        if !response.status().is_success() {
            return Err(ClientError::GetDriftAlertError);
        }

        // Parse response body
        let body: serde_json::Value = response.json()?;

        // Extract alerts from response
        let alerts = body
            .get("alerts")
            .map(|alerts| {
                serde_json::from_value::<Vec<Alert>>(alerts.clone()).inspect_err(|e| {
                    error!(
                        "Failed to parse drift alerts {:?}. Error: {:?}",
                        &request, e
                    );
                })
            })
            .unwrap_or_else(|| {
                error!("No alerts found in response");
                Ok(Vec::new())
            })?;

        Ok(alerts)
    }

    pub fn get_drift_profile(
        &self,
        request: GetProfileRequest,
    ) -> Result<DriftProfile, ClientError> {
        let query_string = serde_qs::to_string(&request)?;

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
            return Err(ClientError::GetDriftProfileError);
        }

        // Get response body
        let body = response.bytes()?;

        // Parse JSON response
        let profile: DriftProfile = serde_json::from_slice(&body)?;

        Ok(profile)
    }

    /// Check if the scouter server is healthy
    pub fn check_service_health(&self) -> Result<bool, ClientError> {
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

    pub fn get_paginated_traces(
        &self,
        request: &TraceFilters,
    ) -> Result<TracePaginationResponse, ClientError> {
        let response = self.client.request(
            Routes::PaginatedTraces,
            RequestType::Put,
            Some(serde_json::to_value(request).unwrap()),
            None,
            None,
        )?;

        if !response.status().is_success() {
            error!(
                "Failed to get paginated traces. Status: {:?}",
                response.status()
            );
            return Err(ClientError::GetPaginatedTracesError);
        }

        // Get response body
        let body = response.bytes()?;

        // Parse JSON response
        let response: TracePaginationResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    pub fn refresh_trace_summary(&self) -> Result<bool, ClientError> {
        let response = self.client.request(
            Routes::RefreshTraceSummary,
            RequestType::Get,
            None,
            None,
            None,
        )?;
        if !response.status().is_success() {
            error!(
                "Failed to refresh trace summary. Status: {:?}",
                response.status()
            );
            return Err(ClientError::RefreshTraceSummaryError);
        }

        Ok(true)
    }

    pub fn get_trace_spans(&self, trace_id: &str) -> Result<TraceSpansResponse, ClientError> {
        let trace_request = TraceRequest {
            trace_id: trace_id.to_string(),
        };
        let response = self.client.request(
            Routes::TraceSpans,
            RequestType::Get,
            Some(serde_json::to_value(&trace_request).unwrap()),
            None,
            None,
        )?;
        if !response.status().is_success() {
            error!("Failed to get trace spans. Status: {:?}", response.status());
            return Err(ClientError::GetTraceSpansError);
        }

        // Get response body
        let body = response.bytes()?;
        // Parse JSON response
        let response: TraceSpansResponse = serde_json::from_slice(&body)?;
        Ok(response)
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
    pub fn new(config: Option<&Bound<'_, PyAny>>) -> Result<Self, ClientError> {
        let config = config.map_or(Ok(HTTPConfig::default()), |unwrapped| {
            if unwrapped.is_instance_of::<HTTPConfig>() {
                unwrapped.extract::<HTTPConfig>()
            } else {
                Err(ClientError::InvalidConfigTypeError.into())
            }
        })?;

        let client = ScouterClient::new(Some(config.clone()))?;

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
        &self,
        profile: &Bound<'_, PyAny>,
        set_active: bool,
        deactivate_others: bool,
    ) -> Result<bool, ClientError> {
        let request = profile
            .call_method0("create_profile_request")?
            .extract::<ProfileRequest>()?;

        let profile_response = self.client.insert_profile(&request)?;

        // update config args
        profile.call_method1(
            "update_config_args",
            (
                Some(profile_response.space),
                Some(profile_response.name),
                Some(profile_response.version),
            ),
        )?;

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

            self.client.update_profile_status(&request)?;
        }

        Ok(true)
    }

    /// Update the status of a profile
    ///
    /// # Arguments
    /// * `request` - A profile status request object
    ///
    /// # Returns
    /// * `Ok(())` if the profile status was updated successfully
    pub fn update_profile_status(
        &self,
        request: ProfileStatusRequest,
    ) -> Result<bool, ClientError> {
        self.client.update_profile_status(&request)
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
        &self,
        py: Python<'py>,
        drift_request: DriftRequest,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        match drift_request.drift_type {
            DriftType::Spc => {
                PyScouterClient::get_spc_binned_drift(py, &self.client.client, drift_request)
            }
            DriftType::Psi => {
                PyScouterClient::get_psi_binned_drift(py, &self.client.client, drift_request)
            }
            DriftType::Custom => {
                PyScouterClient::get_custom_binned_drift(py, &self.client.client, drift_request)
            }
            DriftType::LLM => {
                PyScouterClient::get_llm_metric_binned_drift(py, &self.client.client, drift_request)
            }
        }
    }

    pub fn get_alerts(&self, request: DriftAlertRequest) -> Result<Vec<Alert>, ClientError> {
        debug!("Getting alerts for: {:?}", request);

        let alerts = self.client.get_alerts(&request)?;

        Ok(alerts)
    }

    #[pyo3(signature = (request, path))]
    pub fn download_profile(
        &self,
        request: GetProfileRequest,
        path: Option<PathBuf>,
    ) -> Result<String, ClientError> {
        debug!("Downloading profile: {:?}", request);

        let filename = format!(
            "{}_{}_{}_{}.json",
            request.name, request.space, request.version, request.drift_type
        );

        let profile = self.client.get_drift_profile(request)?;

        PyHelperFuncs::save_to_json(profile, path.clone(), &filename)?;

        Ok(path.map_or(filename, |p| p.to_string_lossy().to_string()))
    }

    /// Update the status of a profile
    ///
    /// # Arguments
    /// * `request` - A profile status request object
    ///
    /// # Returns
    /// * `Ok(())` if the profile status was updated successfully
    pub fn update_paginated_traces(
        &self,
        request: ProfileStatusRequest,
    ) -> Result<bool, ClientError> {
        self.client.update_profile_status(&request)
    }
}

impl PyScouterClient {
    fn get_spc_binned_drift<'py>(
        py: Python<'py>,
        client: &HTTPClient,
        drift_request: DriftRequest,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        let query_string = serde_qs::to_string(&drift_request)?;

        let response = client.request(
            Routes::SpcDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(ClientError::GetDriftDataError);
        }

        let body = response.bytes()?;

        let results: SpcDriftFeatures = serde_json::from_slice(&body)?;

        Ok(results.into_bound_py_any(py).unwrap())
    }
    fn get_psi_binned_drift<'py>(
        py: Python<'py>,
        client: &HTTPClient,
        drift_request: DriftRequest,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        let query_string = serde_qs::to_string(&drift_request)?;

        let response = client.request(
            Routes::PsiDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            // print response text
            error!(
                "Failed to get PSI drift data. Status: {:?}",
                response.status()
            );
            error!("Response text: {:?}", response.text());
            return Err(ClientError::GetDriftDataError);
        }

        let body = response.bytes()?;

        let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&body)?;

        Ok(results.into_bound_py_any(py).unwrap())
    }

    fn get_custom_binned_drift<'py>(
        py: Python<'py>,
        client: &HTTPClient,
        drift_request: DriftRequest,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        let query_string = serde_qs::to_string(&drift_request)?;

        let response = client.request(
            Routes::CustomDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(ClientError::GetDriftDataError);
        }

        let body = response.bytes()?;

        let results: BinnedMetrics = serde_json::from_slice(&body)?;

        Ok(results.into_bound_py_any(py).unwrap())
    }

    fn get_llm_metric_binned_drift<'py>(
        py: Python<'py>,
        client: &HTTPClient,
        drift_request: DriftRequest,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        let query_string = serde_qs::to_string(&drift_request)?;

        let response = client.request(
            Routes::LLMDrift,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if response.status().is_client_error() || response.status().is_server_error() {
            return Err(ClientError::GetDriftDataError);
        }

        let body = response.bytes()?;

        let results: BinnedMetrics = serde_json::from_slice(&body)?;

        Ok(results.into_bound_py_any(py).unwrap())
    }
}
