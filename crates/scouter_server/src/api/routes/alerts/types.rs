use scouter_types::alert::Alert;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UpdateAlertResponse {
    pub updated: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alerts {
    pub alerts: Vec<Alert>,
}
