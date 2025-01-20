use chrono::NaiveDateTime;
use pyo3::prelude::*;
use scouter_types::{DriftType, TimeInterval};
use serde::Deserialize;
use serde::Serialize;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetProfileRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub drift_type: DriftType,
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub time_interval: TimeInterval,
    pub max_data_points: i32,
    pub drift_type: DriftType,
}

#[pymethods]
impl DriftRequest {
    #[new]
    #[pyo3(signature = (name, repository, version, time_interval, max_data_points, drift_type))]
    pub fn new(
        name: String,
        repository: String,
        version: String,
        time_interval: TimeInterval,
        max_data_points: i32,
        drift_type: DriftType,
    ) -> Self {
        DriftRequest {
            name,
            repository,
            version,
            time_interval,
            max_data_points,
            drift_type,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileRequest {
    pub drift_type: DriftType,
    pub profile: String,
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileStatusRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftAlertRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub limit_datetime: Option<NaiveDateTime>,
    pub active: Option<bool>,
    pub limit: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceInfo {
    pub repository: String,
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObservabilityMetricRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub time_interval: String,
    pub max_data_points: i32,
}
