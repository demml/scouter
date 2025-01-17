use chrono::NaiveDateTime;
use pyo3::prelude::*;
use scouter_types::{DriftType, TimeInterval};
use serde::Deserialize;
use serde::Serialize;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub time_window: TimeInterval,
    pub max_data_points: i32,
}

#[pymethods]
impl DriftRequest {
    #[new]
    #[pyo3(signature = (name, repository, version, time_window, max_data_points))]
    pub fn new(
        name: String,
        repository: String,
        version: String,
        time_window: TimeInterval,
        max_data_points: i32,
    ) -> Self {
        DriftRequest {
            name,
            repository,
            version,
            time_window,
            max_data_points,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileRequest {
    pub drift_type: DriftType,
    pub profile: String,
}

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
    pub time_window: String,
    pub max_data_points: i32,
}
