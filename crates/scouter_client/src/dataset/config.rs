use std::sync::Arc;

use arrow::datatypes::SchemaRef;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_types::dataset::{
    fingerprint_from_json_schema, inject_system_columns, json_schema_to_arrow, DatasetError,
    DatasetFingerprint, DatasetNamespace,
};

use super::error::DatasetClientError;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Serialize a Python dict (from `Model.model_json_schema()`) to a JSON string.
pub(crate) fn to_json_str(schema: &Bound<'_, PyAny>) -> Result<String, DatasetError> {
    let json = schema.py().import("json")?;
    let dumped = json.call_method1("dumps", (schema,)).map_err(|_| {
        DatasetError::SchemaParseError(
            "schema must be a JSON-serialisable dict — pass Model.model_json_schema(), not a model instance".to_string(),
        )
    })?;
    Ok(dumped.extract::<String>()?)
}

// ---------------------------------------------------------------------------
// TableConfig
// ---------------------------------------------------------------------------

/// Required configuration for a dataset table.
///
/// Eagerly computes Arrow schema and fingerprint at construction from a
/// Pydantic `BaseModel` class.
#[pyclass]
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TableConfig {
    #[pyo3(get)]
    pub catalog: String,
    #[pyo3(get)]
    pub schema_name: String,
    #[pyo3(get)]
    pub table: String,
    #[pyo3(get)]
    pub partition_columns: Vec<String>,

    // Rust-only derived state
    pub(crate) namespace: DatasetNamespace,
    pub(crate) schema: SchemaRef,
    pub(crate) fingerprint: DatasetFingerprint,
    pub(crate) json_schema: String,
}

#[pymethods]
impl TableConfig {
    #[new]
    #[pyo3(signature = (model, catalog, schema_name, table, partition_columns=None))]
    fn new(
        model: &Bound<'_, PyAny>,
        catalog: String,
        schema_name: String,
        table: String,
        partition_columns: Option<Vec<String>>,
    ) -> Result<Self, DatasetClientError> {
        let partition_columns = partition_columns.unwrap_or_default();

        // model is a Pydantic BaseModel *class* — call model_json_schema() on it
        let schema_dict = model.call_method0("model_json_schema")?;
        let json_schema = to_json_str(&schema_dict)?;

        // Pydantic JSON Schema -> Arrow Schema
        let arrow_schema = json_schema_to_arrow(&json_schema)?;
        let arrow_schema = inject_system_columns(arrow_schema)?;
        let schema: SchemaRef = Arc::new(arrow_schema);

        // Stable fingerprint from the JSON schema
        let fingerprint = fingerprint_from_json_schema(&json_schema)?;

        // Validate namespace components
        let namespace = DatasetNamespace::new(&catalog, &schema_name, &table)?;

        Ok(Self {
            catalog,
            schema_name,
            table,
            partition_columns,
            namespace,
            schema,
            fingerprint,
            json_schema,
        })
    }

    /// The schema fingerprint as a hex string.
    #[getter]
    fn fingerprint_str(&self) -> String {
        self.fingerprint.as_str().to_owned()
    }

    /// Fully-qualified table name: `catalog.schema_name.table`.
    #[getter]
    fn fqn(&self) -> String {
        self.namespace.fqn()
    }

    #[staticmethod]
    fn parse_schema<'py>(
        py: Python<'py>,
        schema: &Bound<'_, PyAny>,
    ) -> Result<Bound<'py, PyDict>, DatasetClientError> {
        let json_str = to_json_str(schema)?;
        let arrow_schema = json_schema_to_arrow(&json_str)?;
        let arrow_schema = inject_system_columns(arrow_schema)?;

        let result = PyDict::new(py);
        for field in arrow_schema.fields() {
            let field_info = PyDict::new(py);
            field_info.set_item("arrow_type", field.data_type().to_string())?;
            field_info.set_item("nullable", field.is_nullable())?;
            result.set_item(field.name(), field_info)?;
        }
        Ok(result)
    }

    #[staticmethod]
    fn compute_fingerprint(schema: &Bound<'_, PyAny>) -> Result<String, DatasetClientError> {
        let json_str = to_json_str(schema)?;
        Ok(fingerprint_from_json_schema(&json_str)?.0)
    }
}

// ---------------------------------------------------------------------------
// WriteConfig
// ---------------------------------------------------------------------------

/// Optional write-side configuration with sensible defaults.
#[pyclass]
#[derive(Clone, Debug)]
pub struct WriteConfig {
    #[pyo3(get)]
    pub batch_size: usize,
    #[pyo3(get)]
    pub scheduled_delay_secs: u64,
}

impl Default for WriteConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            scheduled_delay_secs: 30,
        }
    }
}

#[pymethods]
impl WriteConfig {
    #[new]
    #[pyo3(signature = (batch_size=1000, scheduled_delay_secs=30))]
    fn new(batch_size: usize, scheduled_delay_secs: u64) -> Self {
        Self {
            batch_size: batch_size.max(1),
            scheduled_delay_secs,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_config_defaults() {
        let cfg = WriteConfig::default();
        assert_eq!(cfg.batch_size, 1000);
        assert_eq!(cfg.scheduled_delay_secs, 30);
    }

    #[test]
    fn test_write_config_new_with_defaults() {
        let cfg = WriteConfig::new(1000, 30);
        assert_eq!(cfg.batch_size, 1000);
        assert_eq!(cfg.scheduled_delay_secs, 30);
    }

    #[test]
    fn test_write_config_custom_values() {
        let cfg = WriteConfig::new(500, 60);
        assert_eq!(cfg.batch_size, 500);
        assert_eq!(cfg.scheduled_delay_secs, 60);
    }

    #[test]
    fn test_write_config_clamps_batch_size_zero() {
        let cfg = WriteConfig::new(0, 30);
        assert_eq!(cfg.batch_size, 1);
    }

    #[test]
    fn test_write_config_batch_size_one_is_valid() {
        let cfg = WriteConfig::new(1, 30);
        assert_eq!(cfg.batch_size, 1);
    }
}
