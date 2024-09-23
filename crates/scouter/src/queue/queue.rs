use crate::utils::types::DriftProfile;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorQueue {
    #[pyo3(get)]
    pub drift_profile: DriftProfile,
    
}
