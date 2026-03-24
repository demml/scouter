use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dataset::error::DatasetError;

fn validate_namespace_component(name: &str, label: &str) -> Result<(), DatasetError> {
    if name.is_empty() {
        return Err(DatasetError::SchemaParseError(format!(
            "{label} must not be empty"
        )));
    }
    if name.contains('/') || name.contains("..") || name.contains('"') {
        return Err(DatasetError::SchemaParseError(format!(
            "{label} must not contain '/', '..', or '\"'"
        )));
    }
    Ok(())
}

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
    ) -> Result<Self, DatasetError> {
        let catalog = catalog.into();
        let schema_name = schema_name.into();
        let table = table.into();
        validate_namespace_component(&catalog, "catalog")?;
        validate_namespace_component(&schema_name, "schema_name")?;
        validate_namespace_component(&table, "table")?;
        Ok(Self {
            catalog,
            schema_name,
            table,
        })
    }

    pub fn fqn(&self) -> String {
        format!("{}.{}.{}", self.catalog, self.schema_name, self.table)
    }

    pub fn quoted_fqn(&self) -> String {
        format!(
            "\"{}\".\"{}\".\"{}\"",
            self.catalog, self.schema_name, self.table
        )
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
    /// Uses SHA-256, truncated to 32 hex chars (128 bits) for compactness.
    pub fn from_schema_json(arrow_schema_json: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(arrow_schema_json.as_bytes());
        let hash = hasher.finalize();
        let hex = hex::encode(hash);
        DatasetFingerprint(hex[..32].to_string())
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

impl std::str::FromStr for DatasetStatus {
    type Err = DatasetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(DatasetStatus::Active),
            "deprecated" => Ok(DatasetStatus::Deprecated),
            other => Err(DatasetError::SchemaParseError(format!(
                "Unknown dataset status: '{}'",
                other
            ))),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ns() -> DatasetNamespace {
        DatasetNamespace::new("cat", "sch", "tbl").unwrap()
    }

    #[test]
    fn test_fqn() {
        assert_eq!(make_ns().fqn(), "cat.sch.tbl");
    }

    #[test]
    fn test_storage_path() {
        assert_eq!(make_ns().storage_path(), "datasets/cat/sch/tbl");
    }

    #[test]
    fn test_namespace_rejects_path_traversal() {
        assert!(DatasetNamespace::new("../../etc", "sch", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "../etc", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "sch", "../../etc").is_err());
    }

    #[test]
    fn test_namespace_rejects_slash() {
        assert!(DatasetNamespace::new("a/b", "sch", "tbl").is_err());
    }

    #[test]
    fn test_namespace_rejects_double_quote() {
        assert!(DatasetNamespace::new("a\"b", "sch", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "s\"ch", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "sch", "tb\"l").is_err());
    }

    #[test]
    fn test_quoted_fqn() {
        assert_eq!(make_ns().quoted_fqn(), r#""cat"."sch"."tbl""#);
        let ns = DatasetNamespace::new("my-catalog", "my-schema", "my-table").unwrap();
        assert_eq!(ns.quoted_fqn(), r#""my-catalog"."my-schema"."my-table""#);
    }

    #[test]
    fn test_namespace_rejects_empty() {
        assert!(DatasetNamespace::new("", "sch", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "", "tbl").is_err());
        assert!(DatasetNamespace::new("cat", "sch", "").is_err());
    }

    #[test]
    fn test_fingerprint_is_32_chars() {
        let fp = DatasetFingerprint::from_schema_json("test");
        assert_eq!(fp.as_str().len(), 32);
    }

    #[test]
    fn test_fingerprint_stability() {
        let fp1 = DatasetFingerprint::from_schema_json("test");
        let fp2 = DatasetFingerprint::from_schema_json("test");
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_dataset_status_display() {
        assert_eq!(DatasetStatus::Active.to_string(), "active");
        assert_eq!(DatasetStatus::Deprecated.to_string(), "deprecated");
    }

    #[test]
    fn test_registration_defaults() {
        let ns = make_ns();
        let fp = DatasetFingerprint::from_schema_json("s");
        let reg = DatasetRegistration::new(ns, fp, "{}".into(), "{}".into(), vec![]);
        assert_eq!(reg.status, DatasetStatus::Active);
        assert_eq!(reg.created_at, reg.updated_at);
    }
}
