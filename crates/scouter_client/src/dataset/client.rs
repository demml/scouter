use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_types::dataset::{
    fingerprint_from_json_schema, inject_system_columns, json_schema_to_arrow, DatasetError,
};

fn to_json_str(schema: &Bound<'_, PyAny>) -> Result<String, DatasetError> {
    let json = schema.py().import("json")?;
    let dumped = json
        .call_method1("dumps", (schema,))
        .map_err(|_| {
            DatasetError::SchemaParseError(
                "schema must be a JSON-serialisable dict — pass `Model.model_json_schema()`, not a model instance".to_string(),
            )
        })?;
    Ok(dumped.extract::<String>()?)
}

#[pyclass]
pub struct DatasetClient;

#[pymethods]
impl DatasetClient {
    #[staticmethod]
    pub fn parse_schema<'py>(
        py: Python<'py>,
        schema: &Bound<'_, PyAny>,
    ) -> Result<Bound<'py, PyDict>, DatasetError> {
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
    pub fn compute_fingerprint(schema: &Bound<'_, PyAny>) -> Result<String, DatasetError> {
        let json_str = to_json_str(schema)?;
        Ok(fingerprint_from_json_schema(&json_str)?.0)
    }
}
