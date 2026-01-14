use std::fmt::Display;

use crate::error::{ContractError, TypeError};
use crate::sql::{TraceListItem, TraceMetricBucket, TraceSpan};
use crate::{Alert, GenAIEvalTaskResult, GenAIEvalWorkflowResult};
use crate::{
    CustomInterval, DriftProfile, GenAIEvalRecord, Status, Tag, TagRecord, TraceBaggageRecord,
};
use crate::{DriftType, PyHelperFuncs, TimeInterval};
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use scouter_semver::VersionType;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use tracing::error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListProfilesRequest {
    pub space: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListedProfile {
    pub profile: DriftProfile,
    pub active: bool,
}

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
    // this is the uid for a specific space, name, version, drift_type profile
    pub uid: String,
    // This is the space name. Used for permission checks
    pub space: String,
    pub time_interval: TimeInterval,
    pub max_data_points: i32,
    pub start_custom_datetime: Option<DateTime<Utc>>,
    pub end_custom_datetime: Option<DateTime<Utc>>,
}

#[pymethods]
impl DriftRequest {
    #[new]
    #[pyo3(signature = (uid, space, time_interval, max_data_points, start_datetime=None, end_datetime=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uid: String,
        space: String,
        time_interval: TimeInterval,
        max_data_points: i32,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Result<Self, ContractError> {
        // validate time interval
        let custom_interval = match (start_datetime, end_datetime) {
            (Some(begin), Some(end)) => Some(CustomInterval::new(begin, end)?),
            _ => None,
        };

        Ok(DriftRequest {
            uid,
            space,
            time_interval,
            max_data_points,
            start_custom_datetime: custom_interval.as_ref().map(|interval| interval.begin),
            end_custom_datetime: custom_interval.as_ref().map(|interval| interval.end),
        })
    }
}

impl DriftRequest {
    pub fn has_custom_interval(&self) -> bool {
        self.start_custom_datetime.is_some() && self.end_custom_datetime.is_some()
    }

    pub fn to_custom_interval(&self) -> Option<CustomInterval> {
        if self.has_custom_interval() {
            Some(
                CustomInterval::new(
                    self.start_custom_datetime.unwrap(),
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
    pub version_request: Option<VersionRequest>,

    #[serde(default)]
    pub active: bool,

    #[serde(default)]
    pub deactivate_others: bool,
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

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[pyclass]
pub struct DriftAlertPaginationRequest {
    pub uid: String,
    pub active: Option<bool>,
    pub limit: Option<i32>,
    pub cursor_created_at: Option<DateTime<Utc>>,
    pub cursor_id: Option<i32>,
    pub direction: Option<String>, // "next" or "previous"
    pub start_datetime: Option<DateTime<Utc>>,
    pub end_datetime: Option<DateTime<Utc>>,
}

#[pymethods]
impl DriftAlertPaginationRequest {
    #[allow(clippy::too_many_arguments)]
    #[new]
    #[pyo3(signature = (uid, active=None, limit=None, cursor_created_at=None, cursor_id=None, direction=None, start_datetime=None, end_datetime=None))]
    pub fn new(
        uid: String,
        active: Option<bool>,
        limit: Option<i32>,
        cursor_created_at: Option<DateTime<Utc>>,
        cursor_id: Option<i32>,
        direction: Option<String>,
        start_datetime: Option<DateTime<Utc>>,
        end_datetime: Option<DateTime<Utc>>,
    ) -> Self {
        DriftAlertPaginationRequest {
            uid,
            active,
            limit,
            cursor_created_at,
            cursor_id,
            direction,
            start_datetime,
            end_datetime,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[pyclass]
pub struct RecordCursor {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,

    #[pyo3(get)]
    pub id: i64,
}

#[pymethods]
impl RecordCursor {
    #[new]
    pub fn new(created_at: DateTime<Utc>, id: i64) -> Self {
        RecordCursor { created_at, id }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct DriftAlertPaginationResponse {
    #[pyo3(get)]
    pub items: Vec<Alert>,

    #[pyo3(get)]
    pub has_next: bool,

    #[pyo3(get)]
    pub next_cursor: Option<RecordCursor>,

    #[pyo3(get)]
    pub has_previous: bool,

    #[pyo3(get)]
    pub previous_cursor: Option<RecordCursor>,
}

#[pymethods]
impl DriftAlertPaginationResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ServiceInfo {
    pub space: String,
    pub uid: String,
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
    pub uid: String,
    pub time_interval: String,
    pub max_data_points: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAlertStatus {
    pub id: i32, // this is the unique id for the alert record, not entity_id
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
    pub fn query_tags_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to query tags: {e}");
        ScouterServerError { error: msg }
    }

    pub fn get_baggage_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to get trace baggage records: {e}");
        ScouterServerError { error: msg }
    }

    pub fn get_paginated_traces_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to get paginated traces: {e}");
        ScouterServerError { error: msg }
    }

    pub fn get_trace_spans_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to get trace spans: {e}");
        ScouterServerError { error: msg }
    }

    pub fn get_trace_metrics_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to get trace metrics: {e}");
        ScouterServerError { error: msg }
    }

    pub fn insert_tags_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to insert tags: {e}");
        ScouterServerError { error: msg }
    }

    pub fn get_entity_id_by_tags_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to get entity IDs by tags: {e}");
        ScouterServerError { error: msg }
    }

    pub fn refresh_trace_summary_error<T: Display>(e: T) -> Self {
        let msg = format!("Failed to refresh trace summary: {e}");
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
    pub uid: String,
    pub status: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAlertResponse {
    pub updated: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenAIEvalTaskRequest {
    pub record_uid: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenAIEvalTaskResponse {
    pub tasks: Vec<GenAIEvalTaskResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GenAIEvalRecordPaginationRequest {
    pub service_info: ServiceInfo,
    pub status: Option<Status>,
    pub limit: Option<i32>,
    pub cursor_created_at: Option<DateTime<Utc>>,
    pub cursor_id: Option<i64>,
    pub direction: Option<String>,
    pub start_datetime: Option<DateTime<Utc>>,
    pub end_datetime: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenAIEvalRecordPaginationResponse {
    pub items: Vec<GenAIEvalRecord>,
    pub has_next: bool,
    pub next_cursor: Option<RecordCursor>,
    pub has_previous: bool,
    pub previous_cursor: Option<RecordCursor>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GenAIEvalWorkflowPaginationResponse {
    pub items: Vec<GenAIEvalWorkflowResult>,
    pub has_next: bool,
    pub next_cursor: Option<RecordCursor>,
    pub has_previous: bool,
    pub previous_cursor: Option<RecordCursor>,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TagsRequest {
    pub entity_type: String,
    pub entity_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityIdTagsRequest {
    pub entity_type: String,
    pub tags: Vec<Tag>,
    pub match_all: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityIdTagsResponse {
    pub entity_id: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InsertTagsRequest {
    pub tags: Vec<TagRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceRequest {
    pub trace_id: String,
    pub service_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceMetricsRequest {
    pub service_name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub bucket_interval: String,
    pub attribute_filters: Option<Vec<String>>,
}

#[pymethods]
impl TraceMetricsRequest {
    #[new]
    #[pyo3(signature = (start_time, end_time, bucket_interval,service_name=None, attribute_filters=None))]
    pub fn new(
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval: String,
        service_name: Option<String>,
        attribute_filters: Option<Vec<String>>,
    ) -> Self {
        TraceMetricsRequest {
            service_name,
            start_time,
            end_time,
            bucket_interval,
            attribute_filters,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TracePaginationResponse {
    #[pyo3(get)]
    pub items: Vec<TraceListItem>,

    #[pyo3(get)]
    pub has_next: bool,

    #[pyo3(get)]
    pub next_cursor: Option<TraceCursor>,

    #[pyo3(get)]
    pub has_previous: bool,

    #[pyo3(get)]
    pub previous_cursor: Option<TraceCursor>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceCursor {
    #[pyo3(get)]
    pub start_time: DateTime<Utc>,

    #[pyo3(get)]
    pub trace_id: String,
}

#[pymethods]
impl TraceCursor {
    #[new]
    pub fn new(start_time: DateTime<Utc>, trace_id: String) -> Self {
        TraceCursor {
            start_time,
            trace_id,
        }
    }
}

#[pymethods]
impl TracePaginationResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceBaggageResponse {
    #[pyo3(get)]
    pub baggage: Vec<TraceBaggageRecord>,
}

#[pymethods]
impl TraceBaggageResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceSpansResponse {
    #[pyo3(get)]
    pub spans: Vec<TraceSpan>,
}

#[pymethods]
impl TraceSpansResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn get_span_by_name(&self, span_name: &str) -> Result<Option<TraceSpan>, TypeError> {
        let span = self
            .spans
            .iter()
            .find(|s| s.span_name == span_name)
            .cloned();
        Ok(span)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TraceMetricsResponse {
    #[pyo3(get)]
    pub metrics: Vec<TraceMetricBucket>,
}
#[pymethods]
impl TraceMetricsResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[pyclass]
pub struct TagsResponse {
    #[pyo3(get)]
    pub tags: Vec<TagRecord>,
}

#[pymethods]
impl TagsResponse {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}
