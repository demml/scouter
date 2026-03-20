use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_types::dataset::{
    fingerprint_from_json_schema, inject_system_columns, json_schema_to_arrow, DatasetError,
};

fn to_json_str(schema: &Bound<'_, PyAny>) -> PyResult<String> {
    let json = schema.py().import("json")?;
    json.call_method1("dumps", (schema,))?.extract()
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
        let arrow_schema = inject_system_columns(arrow_schema);

        let result = PyDict::new(py);
        for field in arrow_schema.fields() {
            let field_info = PyDict::new(py);
            field_info.set_item("arrow_type", format!("{}", field.data_type()))?;
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
