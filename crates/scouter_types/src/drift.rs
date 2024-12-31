use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DriftType {
    Spc,
    Psi,
    Custom,
}

#[pymethods]
impl DriftType {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<DriftType> {
        match value.to_lowercase().as_str() {
            "spc" => Some(DriftType::Spc),
            "psi" => Some(DriftType::Psi),
            "custom" => Some(DriftType::Custom),
            _ => None,
        }
    }

    #[getter]
    pub fn to_string(&self) -> &str {
        match self {
            DriftType::Spc => "Spc",
            DriftType::Psi => "Psi",
            DriftType::Custom => "Custom",
        }
    }
}

impl FromStr for DriftType {
    type Err = ScouterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "spc" => Ok(DriftType::Spc),
            "psi" => Ok(DriftType::Psi),
            "custom" => Ok(DriftType::Custom),
            _ => Err(ScouterError::InvalidDriftTypeError(value.to_string())),
        }
    }
}
