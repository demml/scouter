use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use scouter_error::ScouterError;
use scouter_types::{DriftType, TimeInterval};
use serde::Deserialize;
use serde::Serialize;
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
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub time_interval: TimeInterval,
    pub max_data_points: i32,
    pub drift_type: DriftType,
}

#[pymethods]
impl DriftRequest {
    #[new]
    #[pyo3(signature = (name, space, version, time_interval, max_data_points, drift_type))]
    pub fn new(
        name: String,
        space: String,
        version: String,
        time_interval: TimeInterval,
        max_data_points: i32,
        drift_type: DriftType,
    ) -> Self {
        DriftRequest {
            name,
            space,
            version,
            time_interval,
            max_data_points,
            drift_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileRequest {
    pub space: String,
    pub drift_type: DriftType,
    pub profile: String,
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileStatusRequest {
    pub name: String,
    pub space: String,
    pub version: String,
    pub active: bool,
    pub drift_type: Option<DriftType>,
}

#[pymethods]
impl ProfileStatusRequest {
    #[new]
    #[pyo3(signature = (name, space, version, drift_type=None, active=false))]
    pub fn new(
        name: String,
        space: String,
        version: String,
        drift_type: Option<DriftType>,
        active: bool,
    ) -> Self {
        ProfileStatusRequest {
            name,
            space,
            version,
            active,
            drift_type,
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
}

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

    pub fn create_user_error(e: ScouterError) -> Self {
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

    pub fn get_user_error(e: ScouterError) -> Self {
        error!("Failed to get user: {}", e);
        ScouterServerError {
            error: "Failed to get user".to_string(),
        }
    }

    pub fn list_users_error(e: ScouterError) -> Self {
        error!("Failed to list users: {}", e);
        ScouterServerError {
            error: "Failed to list users".to_string(),
        }
    }

    pub fn update_user_error(e: ScouterError) -> Self {
        error!("Failed to update user: {}", e);
        ScouterServerError {
            error: "Failed to update user".to_string(),
        }
    }
    pub fn delete_user_error(e: ScouterError) -> Self {
        error!("Failed to delete user: {}", e);
        ScouterServerError {
            error: "Failed to delete user".to_string(),
        }
    }

    pub fn check_last_admin_error(e: ScouterError) -> Self {
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

    pub fn new(error: String) -> Self {
        ScouterServerError { error }
    }

    pub fn query_records_error(e: ScouterError) -> Self {
        let msg = format!("Failed to query records: {:?}", e);
        ScouterServerError { error: msg }
    }

    pub fn query_alerts_error(e: ScouterError) -> Self {
        let msg = format!("Failed to query alerts: {:?}", e);
        ScouterServerError { error: msg }
    }

    pub fn query_profile_error(e: ScouterError) -> Self {
        let msg = format!("Failed to query profile: {:?}", e);
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
