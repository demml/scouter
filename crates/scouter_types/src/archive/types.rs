use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum StorageType {
    Google,
    Aws,
    Local,
    Azure,
}

#[derive(Debug, PartialEq, Default)]
pub struct ArchiveRecord {
    pub created_at: DateTime<Utc>,
    pub custom: bool,
    pub psi: bool,
    pub spc: bool,
    pub llm_drift: bool,
    pub llm_metric: bool,
}
