use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use scouter_types::{DriftType, TimeInterval};
use serde::Deserialize;
use serde::Serialize;

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
    pub repository: String,
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
