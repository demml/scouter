use crate::custom::CustomDriftProfile;
use crate::psi::PsiDriftProfile;
use crate::spc::SpcDriftProfile;
use crate::util::ProfileBaseArgs;
use crate::{AlertDispatchConfig, ProfileArgs};
use crate::{FileName, ProfileFuncs};
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_error::{PyScouterError, ScouterError};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;
use std::str::FromStr;
use strum_macros::EnumIter;

#[pyclass(eq)]
#[derive(Debug, EnumIter, PartialEq, Serialize, Deserialize, Clone, Default, Eq, Hash)]
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

impl Display for DriftType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftType::Spc => write!(f, "Spc"),
            DriftType::Psi => write!(f, "Psi"),
            DriftType::Custom => write!(f, "Custom"),
        }
    }
}

pub struct DriftArgs {
    pub name: String,
    pub space: String,
    pub version: String,
    pub dispatch_config: AlertDispatchConfig,
}

// Generic enum to be used on scouter server
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DriftProfile {
    Spc(SpcDriftProfile),
    Psi(PsiDriftProfile),
    Custom(CustomDriftProfile),
}

#[pymethods]
impl DriftProfile {
    #[getter]
    pub fn profile<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            DriftProfile::Spc(profile) => profile
                .clone()
                .into_bound_py_any(py)
                .map_err(|e| PyScouterError::new_err(e.to_string())),
            DriftProfile::Psi(profile) => profile
                .clone()
                .into_bound_py_any(py)
                .map_err(|e| PyScouterError::new_err(e.to_string())),
            DriftProfile::Custom(profile) => profile
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
                Ok(DriftProfile::Spc(profile))
            }
            DriftType::Psi => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::Psi(profile))
            }
            DriftType::Custom => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::Custom(profile))
            }
        }
    }

    /// Get the base arguments for a drift profile
    pub fn get_base_args(&self) -> ProfileArgs {
        match self {
            DriftProfile::Spc(profile) => profile.get_base_args(),
            DriftProfile::Psi(profile) => profile.get_base_args(),
            DriftProfile::Custom(profile) => profile.get_base_args(),
        }
    }

    pub fn to_value(&self) -> serde_json::Value {
        match self {
            DriftProfile::Spc(profile) => profile.to_value(),
            DriftProfile::Psi(profile) => profile.to_value(),
            DriftProfile::Custom(profile) => profile.to_value(),
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
                Ok(DriftProfile::Spc(profile))
            }
            DriftType::Psi => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::Psi(profile))
            }
            DriftType::Custom => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::Custom(profile))
            }
        }
    }

    pub fn from_python(
        drift_type: DriftType,
        profile: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError> {
        match drift_type {
            DriftType::Spc => {
                let profile = profile.extract::<SpcDriftProfile>()?;
                Ok(DriftProfile::Spc(profile))
            }
            DriftType::Psi => {
                let profile = profile.extract::<PsiDriftProfile>()?;
                Ok(DriftProfile::Psi(profile))
            }
            DriftType::Custom => {
                let profile = profile.extract::<CustomDriftProfile>()?;
                Ok(DriftProfile::Custom(profile))
            }
        }
    }

    pub fn get_spc_profile(&self) -> Result<&SpcDriftProfile, ScouterError> {
        match self {
            DriftProfile::Spc(profile) => Ok(profile),
            _ => Err(ScouterError::Error(
                "Invalid drift profile type".to_string(),
            )),
        }
    }

    pub fn get_psi_profile(&self) -> Result<&PsiDriftProfile, ScouterError> {
        match self {
            DriftProfile::Psi(profile) => Ok(profile),
            _ => Err(ScouterError::Error(
                "Invalid drift profile type".to_string(),
            )),
        }
    }

    pub fn drift_type(&self) -> DriftType {
        match self {
            DriftProfile::Spc(_) => DriftType::Spc,
            DriftProfile::Psi(_) => DriftType::Psi,
            DriftProfile::Custom(_) => DriftType::Custom,
        }
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }

    pub fn load_from_json(path: PathBuf) -> Result<Self, ScouterError> {
        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;
        serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)
    }
}

impl Default for DriftProfile {
    fn default() -> Self {
        DriftProfile::Spc(SpcDriftProfile::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    #[test]
    fn test_drift_profile_enum() {
        let profile = DriftProfile::Spc(SpcDriftProfile::default());

        // save to temppath
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("profile.json");

        profile.save_to_json(Some(path.clone())).unwrap();

        // assert path exists
        assert!(path.exists());

        // load from path
        let loaded_profile = DriftProfile::load_from_json(path).unwrap();

        // assert profile is the same
        assert_eq!(profile, loaded_profile);
    }
}
