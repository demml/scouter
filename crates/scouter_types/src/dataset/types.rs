use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatasetNamespace {
    pub catalog: String,
    pub schema_name: String,
    pub table: String,
}

impl DatasetNamespace {
    pub fn new(
        catalog: impl Into<String>,
        schema_name: impl Into<String>,
        table: impl Into<String>,
    ) -> Self {
        Self {
            catalog: catalog.into(),
            schema_name: schema_name.into(),
            table: table.into(),
        }
    }

    pub fn fqn(&self) -> String {
        format!("{}.{}.{}", self.catalog, self.schema_name, self.table)
    }

    pub fn storage_path(&self) -> String {
        format!(
            "datasets/{}/{}/{}",
            self.catalog, self.schema_name, self.table
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetFingerprint(pub String);

impl DatasetFingerprint {
    /// Compute a stable fingerprint from the canonical Arrow schema JSON.
    /// Uses SHA-256, truncated to 16 hex chars for compactness.
    pub fn from_schema_json(arrow_schema_json: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(arrow_schema_json.as_bytes());
        let hash = hasher.finalize();
        let hex = hex::encode(hash);
        DatasetFingerprint(hex[..16].to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DatasetFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetStatus {
    Active,
    Deprecated,
}

impl std::fmt::Display for DatasetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatasetStatus::Active => write!(f, "active"),
            DatasetStatus::Deprecated => write!(f, "deprecated"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetRegistration {
    pub namespace: DatasetNamespace,
    pub fingerprint: DatasetFingerprint,
    /// Arrow schema serialized to JSON (IPC schema format)
    pub arrow_schema_json: String,
    /// Original Pydantic JSON Schema for client-side reconstruction
    pub json_schema: String,
    /// User-specified partition columns beyond the default `scouter_partition_date`
    pub partition_columns: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: DatasetStatus,
}

impl DatasetRegistration {
    pub fn new(
        namespace: DatasetNamespace,
        fingerprint: DatasetFingerprint,
        arrow_schema_json: String,
        json_schema: String,
        partition_columns: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            namespace,
            fingerprint,
            arrow_schema_json,
            json_schema,
            partition_columns,
            created_at: now,
            updated_at: now,
            status: DatasetStatus::Active,
        }
    }
}
