use std::fmt::Display;

use crate::error::{ContractError, TypeError};
use crate::llm::PaginationRequest;
use crate::{CustomInterval, Status};
use crate::{DriftType, PyHelperFuncs, TimeInterval};
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use scouter_semver::VersionType;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use tracing::error;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetProfileRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub drift_type: DriftType,
}

#[pymethods]
impl GetProfileRequest {
    #[new]
    #[pyo3(signature = (name, space, version, drift_type))]
    pub fn new(name: String, space: String, version: String, drift_type: DriftType) -> Self {
        GetProfileRequest {
            name,
            space,
            version,
            drift_type,
        }
    }
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DriftRequest {
    pub space: String,
    pub name: String,
    pub version: String,
    pub time_interval: TimeInterval,
    pub max_data_points: i32,
    pub drift_type: DriftType,
    pub begin_custom_datetime: Option<DateTime<Utc>>,
    pub end_custom_datetime: Option<DateTime<Utc>>,
}

#[pymethods]
impl DriftRequest {
    #[new]
    #[pyo3(signature = (name, space, version, time_interval, max_data_points, drift_type, begin_datetime=None, end_datetime=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        space: String,
        version: String,
        time_interval: TimeInterval,
        max_data_points: i32,
        drift_type: DriftType,
        begin_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Self, ContractError> {
        // validate time interval
        let custom_interval = match (begin_datetime, end_datetime) {
            (Some(begin), Some(end)) => Some(CustomInterval::new(begin, end)?),
            _ => None,
        };

        Ok(DriftRequest {
            name,
            space,
            version,
            time_interval,
            max_data_points,
            drift_type,
            begin_custom_datetime: custom_interval.as_ref().map(|interval| interval.start),
            end_custom_datetime: custom_interval.as_ref().map(|interval| interval.end),
        })
    }
}

impl DriftRequest {
    pub fn has_custom_interval(&self) -> bool {
        self.begin_custom_datetime.is_some() && self.end_custom_datetime.is_some()
    }

    pub fn to_custom_interval(&self) -> Option<CustomInterval> {
        if self.has_custom_interval() {
            Some(
                CustomInterval::new(
                    self.begin_custom_datetime.unwrap(),
                    self.end_custom_datetime.unwrap(),
                )
                .unwrap(),
            )
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionRequest {
    pub version: Option<String>,
    pub version_type: VersionType,
    pub pre_tag: Option<String>,
    pub build_tag: Option<String>,
}

impl Default for VersionRequest {
    fn default() -> Self {
        VersionRequest {
            version: None,
            version_type: VersionType::Minor,
            pre_tag: None,
            build_tag: None,
        }
    }
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileRequest {
    pub space: String,
    pub drift_type: DriftType,
    pub profile: String,
    pub version_request: VersionRequest,
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileStatusRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub active: bool,
    pub drift_type: Option<DriftType>,
    pub deactivate_others: bool,
}

#[pymethods]
impl ProfileStatusRequest {
    #[new]
    #[pyo3(signature = (name, space, version, drift_type=None, active=false, deactivate_others=false))]
    pub fn new(
        name: String,
        space: String,
        version: String,
        drift_type: Option<DriftType>,
        active: bool,
        deactivate_others: bool,
    ) -> Self {
        ProfileStatusRequest {
            name,
            space,
            version,
            active,
            drift_type,
            deactivate_others,
        }
    }
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftAlertRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub limit_datetime: Option<DateTime<Utc>>,
    pub active: Option<bool>,
    pub limit: Option<i32>,
}

#[pymethods]
impl DriftAlertRequest {
    #[new]
    #[pyo3(signature = (name, space, version, active=false, limit_datetime=None, limit=None))]
    pub fn new(
        name: String,
        space: String,
        version: String,
        active: bool,
        limit_datetime: Option<DateTime<Utc>>,
        limit: Option<i32>,
    ) -> Self {
        DriftAlertRequest {
            name,
            space,
            version,
            limit_datetime,
            active: Some(active),
            limit,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceInfo {
    pub space: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LLMServiceInfo {
    pub space: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftTaskInfo {
    pub space: String,
    pub name: String,
    pub version: String,
    pub uid: String,
    pub drift_type: DriftType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObservabilityMetricRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub time_interval: String,
    pub max_data_points: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAlertStatus {
    pub id: i32,
    pub active: bool,
    pub space: String,
}

/// Common struct for returning errors from scouter server (axum response)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScouterServerError {
    pub error: String,
}

impl ScouterServerError {
    pub fn permission_denied() -> Self {
        ScouterServerError {
            error: "Permission denied".to_string(),
        }
    }

    pub fn need_admin_permission() -> Self {
        error!("User does not have admin permissions");
        ScouterServerError {
            error: "Need admin permission".to_string(),
        }
    }

    pub fn user_already_exists() -> Self {
        ScouterServerError {
            error: "User already exists".to_string(),
        }
    }

    pub fn create_user_error<T: Display>(e: T) -> Self {
        error!("Failed to create user: {}", e);
        ScouterServerError {
            error: "Failed to create user".to_string(),
        }
    }

    pub fn user_not_found() -> Self {
        ScouterServerError {
            error: "User not found".to_string(),
        }
    }

    pub fn get_user_error<T: Display>(e: T) -> Self {
        error!("Failed to get user: {}", e);
        ScouterServerError {
            error: "Failed to get user".to_string(),
        }
    }

    pub fn list_users_error<T: Display>(e: T) -> Self {
        error!("Failed to list users: {}", e);
        ScouterServerError {
            error: "Failed to list users".to_string(),
        }
    }

    pub fn update_user_error<T: Display>(e: T) -> Self {
        error!("Failed to update user: {}", e);
        ScouterServerError {
            error: "Failed to update user".to_string(),
        }
    }
    pub fn delete_user_error<T: Display>(e: T) -> Self {
        error!("Failed to delete user: {}", e);
        ScouterServerError {
            error: "Failed to delete user".to_string(),
        }
    }

    pub fn check_last_admin_error<T: Display>(e: T) -> Self {
        error!("Failed to check admin status: {}", e);
        ScouterServerError {
            error: "Failed to check admin status".to_string(),
        }
    }

    pub fn cannot_delete_last_admin() -> Self {
        error!("Cannot delete the last admin user");
        ScouterServerError {
            error: "Cannot delete the last admin user".to_string(),
        }
    }
    pub fn username_header_not_found() -> Self {
        error!("Username header not found");
        ScouterServerError {
            error: "Username header not found".to_string(),
        }
    }

    pub fn invalid_username_format() -> Self {
        error!("Invalid username format");
        ScouterServerError {
            error: "Invalid username format".to_string(),
        }
    }

    pub fn password_header_not_found() -> Self {
        error!("Password header not found");
        ScouterServerError {
            error: "Password header not found".to_string(),
        }
    }
    pub fn invalid_password_format() -> Self {
        error!("Invalid password format");
        ScouterServerError {
            error: "Invalid password format".to_string(),
        }
    }

    pub fn user_validation_error() -> Self {
        error!("User validation failed");
        ScouterServerError {
            error: "User validation failed".to_string(),
        }
    }

    pub fn failed_token_validation() -> Self {
        error!("Failed to validate token");
        ScouterServerError {
            error: "Failed to validate token".to_string(),
        }
    }

    pub fn bearer_token_not_found() -> Self {
        error!("Bearer token not found");
        ScouterServerError {
            error: "Bearer token not found".to_string(),
        }
    }

    pub fn refresh_token_error<T: Display>(e: T) -> Self {
        error!("Failed to refresh token: {}", e);
        ScouterServerError {
            error: "Failed to refresh token".to_string(),
        }
    }

    pub fn unauthorized<T: Display>(e: T) -> Self {
        error!("Unauthorized: {}", e);
        ScouterServerError {
            error: "Unauthorized".to_string(),
        }
    }

    pub fn jwt_decode_error(e: String) -> Self {
        error!("Failed to decode JWT token: {}", e);
        ScouterServerError {
            error: "Failed to decode JWT token".to_string(),
        }
    }

    pub fn no_refresh_token() -> Self {
        error!("No refresh token provided");
        ScouterServerError {
            error: "No refresh token provided".to_string(),
        }
    }

    pub fn new(error: String) -> Self {
        ScouterServerError { error }
    }

    pub fn query_records_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to query records: {e}");
        ScouterServerError { error: msg }
    }

    pub fn query_alerts_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to query alerts: {e}");
        ScouterServerError { error: msg }
    }

    pub fn query_profile_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to query profile: {e}");
        ScouterServerError { error: msg }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScouterResponse {
    pub status: String,
    pub message: String,
}

impl ScouterResponse {
    pub fn new(status: String, message: String) -> Self {
        ScouterResponse { status, message }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegisteredProfileResponse {
    pub space: String,
    pub name: String,
    pub version: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAlertResponse {
    pub updated: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LLMDriftRecordPaginationRequest {
    pub service_info: ServiceInfo,
    pub status: Option<Status>,
    pub pagination: PaginationRequest,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedMetricStats {
    #[pyo3(get)]
    pub avg: f64,

    #[pyo3(get)]
    pub lower_bound: f64,

    #[pyo3(get)]
    pub upper_bound: f64,
}

#[pymethods]
impl BinnedMetricStats {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedMetric {
    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub stats: Vec<BinnedMetricStats>,
}

#[pymethods]
impl BinnedMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedMetrics {
    #[pyo3(get)]
    pub metrics: BTreeMap<String, BinnedMetric>,
}

#[pymethods]
impl BinnedMetrics {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        key: &str,
    ) -> Result<Option<Py<BinnedMetric>>, TypeError> {
        match self.metrics.get(key) {
            Some(metric) => {
                let metric = Py::new(py, metric.clone())?;
                Ok(Some(metric))
            }
            None => Ok(None),
        }
    }
}

impl BinnedMetrics {
    pub fn from_vec(metrics: Vec<BinnedMetric>) -> Self {
        let mapped: BTreeMap<String, BinnedMetric> = metrics
            .into_iter()
            .map(|metric| (metric.metric.clone(), metric))
            .collect();
        BinnedMetrics { metrics: mapped }
    }
}
