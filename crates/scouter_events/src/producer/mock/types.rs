use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_types::TransportType;
use tracing::debug;

#[pyclass]
#[derive(Debug, Clone)]
pub struct MockConfig {
    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
impl MockConfig {
    #[new]
    #[pyo3(signature = ( **py_kwargs))]
    pub fn new(py_kwargs: Option<&Bound<'_, PyDict>>) -> Self {
        debug!("Creating MockConfig with kwargs: {py_kwargs:?}");
        MockConfig {
            transport_type: TransportType::Mock,
        }
    }

    pub fn __str__(&self) -> String {
        "MockConfig".to_string()
    }
}

impl Default for MockConfig {
    fn default() -> Self {
        MockConfig::new(None)
    }
}
