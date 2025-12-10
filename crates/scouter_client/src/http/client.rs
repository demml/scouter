#![allow(clippy::useless_conversion)]
use crate::error::ClientError;
use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_settings::http::HttpConfig;
use scouter_types::contracts::{
    DriftAlertRequest, DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
};
use scouter_types::http::{RequestType, Routes};
use scouter_types::sql::TraceFilters;
use scouter_types::{
    RegisteredProfileResponse, TagsRequest, TagsResponse, TraceBaggageResponse,
    TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse, TraceRequest,
    TraceSpansResponse,
};

use crate::http::HttpClient;
use scouter_types::{
    alert::Alert, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures, BinnedMetrics, DriftProfile,
    DriftType, PyHelperFuncs,
};
use std::path::PathBuf;
use tracing::{debug, error};

pub const DOWNLOAD_CHUNK_SIZE: usize = 1024 * 1024 * 5;

#[derive(Debug, Clone)]
pub struct ScouterClient {
    client: HttpClient,
}

impl ScouterClient {
    pub fn new(config: Option<HttpConfig>) -> Result<Self, ClientError> {
        let client = HttpClient::new(config.unwrap_or_default())?;

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

    fn get_paginated_traces(
        &self,
        request: &TraceFilters,
    ) -> Result<TracePaginationResponse, ClientError> {
        let response = self.client.request(
            Routes::PaginatedTraces,
            RequestType::Post,
            Some(serde_json::to_value(request).unwrap()),
            None,
            None,
        )?;

        if !response.status().is_success() {
            let status_code = response.status();
            let err_msg = response.text().unwrap_or_default();
            error!(
                "Failed to get paginated traces. Status: {:?}, Error: {}",
                status_code, err_msg
            );
            return Err(ClientError::GetPaginatedTracesError);
        }

        // Get response body
        let body = response.bytes()?;

        // Parse JSON response
        let response: TracePaginationResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    fn get_trace_spans(
        &self,
        trace_id: &str,
        service_name: Option<&str>,
    ) -> Result<TraceSpansResponse, ClientError> {
        let trace_request = TraceRequest {
            trace_id: trace_id.to_string(),
            service_name: service_name.map(|s| s.to_string()),
        };

        let query_string = serde_qs::to_string(&trace_request)?;

        let response = self.client.request(
            Routes::TraceSpans,
            RequestType::Get,
            None,
            Some(query_string),
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

    fn get_trace_metrics(
        &self,
        request: TraceMetricsRequest,
    ) -> Result<TraceMetricsResponse, ClientError> {
        let query_string = serde_qs::to_string(&request)?;
        let response = self.client.request(
            Routes::TraceMetrics,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;
        if !response.status().is_success() {
            error!(
                "Failed to get trace metrics. Status: {:?}",
                response.status()
            );
            return Err(ClientError::GetTraceMetricsError);
        }

        // Get response body
        let body = response.bytes()?;
        // Parse JSON response
        let response: TraceMetricsResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    fn get_trace_baggage(&self, trace_id: &str) -> Result<TraceBaggageResponse, ClientError> {
        let trace_request = TraceRequest {
            trace_id: trace_id.to_string(),
            service_name: None,
        };
        let query_string = serde_qs::to_string(&trace_request)?;
        let response = self.client.request(
            Routes::TraceBaggage,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;
        if !response.status().is_success() {
            error!(
                "Failed to get trace baggage. Status: {:?}",
                response.status()
            );
            return Err(ClientError::GetTraceBaggageError);
        }

        // Get response body
        let body = response.bytes()?;
        // Parse JSON response
        let response: TraceBaggageResponse = serde_json::from_slice(&body)?;
        Ok(response)
    }

    fn get_tags(&self, tag_request: TagsRequest) -> Result<TagsResponse, ClientError> {
        let query_string = serde_qs::to_string(&tag_request)?;

        let response = self.client.request(
            Routes::Tags,
            RequestType::Get,
            None,
            Some(query_string),
            None,
        )?;

        if !response.status().is_success() {
            error!("Failed to get tags. Status: {:?}", response.status());
            return Err(ClientError::GetTagsError);
        }

        let body = response.bytes()?;

        let tags_response: TagsResponse = serde_json::from_slice(&body)?;

        Ok(tags_response)
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
        let config = config.map_or(Ok(HttpConfig::default()), |unwrapped| {
            if unwrapped.is_instance_of::<HttpConfig>() {
                unwrapped.extract::<HttpConfig>()
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
        let mut request = profile
            .call_method0("create_profile_request")?
            .extract::<ProfileRequest>()
            .inspect_err(|e| error!("Failed to extract profile request: {:?}", e.to_string()))?;

        request.active = set_active;
        request.deactivate_others = deactivate_others;

        let profile_response = self.client.insert_profile(&request)?;

        // update config args
        profile.call_method1(
            "update_config_args",
            (
                Some(profile_response.space),
                Some(profile_response.name),
                Some(profile_response.version),
                Some(profile_response.uid),
            ),
        )?;

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
        drift_type: DriftType,
    ) -> Result<Bound<'py, PyAny>, ClientError> {
        // get drift type
        match drift_type {
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

    /// Get paginated traces from the scouter server
    /// # Arguments
    /// * `filters` - A trace filters object
    /// # Returns
    /// * A trace pagination response object
    pub fn get_paginated_traces(
        &self,
        filters: TraceFilters,
    ) -> Result<TracePaginationResponse, ClientError> {
        self.client.get_paginated_traces(&filters)
    }

    /// Get trace spans for a given trace ID
    /// # Arguments
    /// * `trace_id` - The ID of the trace
    /// # Returns
    /// * A trace spans response object
    #[pyo3(signature = (trace_id, service_name=None))]
    pub fn get_trace_spans(
        &self,
        trace_id: &str,
        service_name: Option<&str>,
    ) -> Result<TraceSpansResponse, ClientError> {
        self.client.get_trace_spans(trace_id, service_name)
    }

    /// Get trace metrics for a given trace metrics request
    /// # Arguments
    /// * `trace_id` - The ID of the trace
    /// # Returns
    /// * A trace baggage response object
    pub fn get_trace_baggage(&self, trace_id: &str) -> Result<TraceBaggageResponse, ClientError> {
        self.client.get_trace_baggage(trace_id)
    }

    /// Get trace metrics for a given trace metrics request
    /// # Arguments
    /// * `request` - A trace metrics request object
    /// # Returns
    /// * A trace metrics response object
    pub fn get_trace_metrics(
        &self,
        request: TraceMetricsRequest,
    ) -> Result<TraceMetricsResponse, ClientError> {
        self.client.get_trace_metrics(request)
    }

    /// Get tags for a given entity type and ID
    /// # Arguments
    /// * `entity_type` - The type of the entity
    /// * `entity_id` - The ID of the entity
    /// # Returns
    /// * A tags response object
    pub fn get_tags(
        &self,
        entity_type: String,
        entity_id: String,
    ) -> Result<TagsResponse, ClientError> {
        let tag_request = TagsRequest {
            entity_type,
            entity_id,
        };
        self.client.get_tags(tag_request)
    }
}

impl PyScouterClient {
    fn get_spc_binned_drift<'py>(
        py: Python<'py>,
        client: &HttpClient,
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
        client: &HttpClient,
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
        client: &HttpClient,
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
        client: &HttpClient,
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
