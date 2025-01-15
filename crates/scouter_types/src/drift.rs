use crate::custom::CustomDriftProfile;
use crate::dispatch::AlertDispatchType;
use crate::psi::PsiDriftProfile;
use crate::spc::SpcDriftProfile;
use crate::util::ProfileBaseArgs;
use crate::ProfileArgs;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_error::{PyScouterError, ScouterError};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub enum DriftType {
    #[default]
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

pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
}

// Generic enum to be used on scouter server
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DriftProfile {
    SpcDriftProfile(SpcDriftProfile),
    PsiDriftProfile(PsiDriftProfile),
    CustomDriftProfile(CustomDriftProfile),
}

#[pymethods]
impl DriftProfile {
    pub fn profile<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            DriftProfile::SpcDriftProfile(profile) => profile
                .clone()
                .into_bound_py_any(py)
                .map_err(|e| PyScouterError::new_err(e.to_string())),
            DriftProfile::PsiDriftProfile(profile) => profile
                .clone()
                .into_bound_py_any(py)
                .map_err(|e| PyScouterError::new_err(e.to_string())),
            DriftProfile::CustomDriftProfile(profile) => profile
                .clone()
                .into_bound_py_any(py)
                .map_err(|e| PyScouterError::new_err(e.to_string())),
        }
    }
}

impl DriftProfile {
    /// Create a new DriftProfile from a DriftType and a profile string
    /// This function will map the drift type to the correct profile type to load
    ///
    /// # Arguments
    ///
    /// * `drift_type` - DriftType enum
    /// * `profile` - Profile string
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - Result of DriftProfile
    pub fn from_str(drift_type: DriftType, profile: String) -> Result<Self, ScouterError> {
        match drift_type {
            DriftType::Spc => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::SpcDriftProfile(profile))
            }
            DriftType::Psi => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::PsiDriftProfile(profile))
            }
            DriftType::Custom => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::CustomDriftProfile(profile))
            }
        }
    }

    /// Get the base arguments for a drift profile
    pub fn get_base_args(&self) -> ProfileArgs {
        match self {
            DriftProfile::SpcDriftProfile(profile) => profile.get_base_args(),
            DriftProfile::PsiDriftProfile(profile) => profile.get_base_args(),
            DriftProfile::CustomDriftProfile(profile) => profile.get_base_args(),
        }
    }

    pub fn to_value(&self) -> serde_json::Value {
        match self {
            DriftProfile::SpcDriftProfile(profile) => profile.to_value(),
            DriftProfile::PsiDriftProfile(profile) => profile.to_value(),
            DriftProfile::CustomDriftProfile(profile) => profile.to_value(),
        }
    }

    /// Create a new DriftProfile from a value (this is used by scouter-server)
    /// This function will map the drift type to the correct profile type to load
    ///
    /// # Arguments
    ///
    /// * `body` - Request body
    /// * `drift_type` - Drift type string
    ///
    pub fn from_value(body: serde_json::Value, drift_type: &str) -> Result<Self, ScouterError> {
        let drift_type = DriftType::from_str(drift_type)
            .map_err(|_| ScouterError::InvalidDriftTypeError(drift_type.to_string()))?;
        match drift_type {
            DriftType::Spc => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::SpcDriftProfile(profile))
            }
            DriftType::Psi => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::PsiDriftProfile(profile))
            }
            DriftType::Custom => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::CustomDriftProfile(profile))
            }
        }
    }
}

impl Default for DriftProfile {
    fn default() -> Self {
        DriftProfile::SpcDriftProfile(SpcDriftProfile::default())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_type_from_str_base() {
        assert_eq!(DriftType::from_str("SPC").unwrap(), DriftType::Spc);
        assert_eq!(DriftType::from_str("PSI").unwrap(), DriftType::Psi);
        assert_eq!(DriftType::from_str("CUSTOM").unwrap(), DriftType::Custom);
        assert!(DriftType::from_str("INVALID").is_err());
    }

    #[test]
    fn test_drift_type_value_base() {
        assert_eq!(DriftType::Spc.to_string(), "Spc");
        assert_eq!(DriftType::Psi.to_string(), "Psi");
        assert_eq!(DriftType::Custom.to_string(), "Custom");
    }
}
