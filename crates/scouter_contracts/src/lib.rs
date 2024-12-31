use scouter::core::drift::base::DriftType;
use serde::Deserialize;
use serde::Serialize;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriftRequest {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub time_window: String,
    pub max_data_points: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProfileRequest {
    pub drift_type: DriftType,
    pub profile: serde_json::Value,
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
    pub limit_timestamp: Option<String>,
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
