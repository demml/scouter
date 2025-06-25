use pyo3::prelude::*;
use scouter_types::TransportType;

#[pyclass]
#[derive(Debug, Clone)]
pub struct MockConfig {
    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
impl MockConfig {
    #[new]
    #[pyo3(signature = ())]
    pub fn new() -> Self {
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
        MockConfig::new()
    }
}
