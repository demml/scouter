use crate::custom::CustomDriftProfile;
use crate::error::ProfileError;
use crate::genai::profile::LLMDriftProfile;
use crate::psi::PsiDriftProfile;
use crate::spc::SpcDriftProfile;
use crate::util::ProfileBaseArgs;
use crate::{AlertDispatchConfig, ProfileArgs};
use crate::{FileName, PyHelperFuncs};
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    GenAI,
}

#[pymethods]
impl DriftType {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<DriftType> {
        match value.to_lowercase().as_str() {
            "spc" => Some(DriftType::Spc),
            "psi" => Some(DriftType::Psi),
            "custom" => Some(DriftType::Custom),
            "genai" => Some(DriftType::GenAI),
            _ => None,
        }
    }

    #[getter]
    pub fn to_string(&self) -> &str {
        match self {
            DriftType::Spc => "Spc",
            DriftType::Psi => "Psi",
            DriftType::Custom => "Custom",
            DriftType::GenAI => "GenAI",
        }
    }
}

impl FromStr for DriftType {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "spc" => Ok(DriftType::Spc),
            "psi" => Ok(DriftType::Psi),
            "custom" => Ok(DriftType::Custom),
            "genai" => Ok(DriftType::GenAI),
            _ => Err(ProfileError::InvalidDriftTypeError),
        }
    }
}

impl Display for DriftType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriftType::Spc => write!(f, "Spc"),
            DriftType::Psi => write!(f, "Psi"),
            DriftType::Custom => write!(f, "Custom"),
            DriftType::GenAI => write!(f, "GenAI"),
        }
    }
}

pub struct DriftArgs {
    pub name: String,
    pub space: String,
    pub version: String,
    pub dispatch_config: AlertDispatchConfig,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DriftProfile {
    Spc(SpcDriftProfile),
    Psi(PsiDriftProfile),
    Custom(CustomDriftProfile),
    GenAI(GenAIDriftProfile),
}

#[pymethods]
impl DriftProfile {
    #[getter]
    pub fn profile<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, ProfileError> {
        match self {
            DriftProfile::Spc(profile) => Ok(profile.clone().into_bound_py_any(py)?),
            DriftProfile::Psi(profile) => Ok(profile.clone().into_bound_py_any(py)?),
            DriftProfile::Custom(profile) => Ok(profile.clone().into_bound_py_any(py)?),
            DriftProfile::GenAI(profile) => Ok(profile.clone().into_bound_py_any(py)?),
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
    pub fn from_str(drift_type: DriftType, profile: String) -> Result<Self, ProfileError> {
        match drift_type {
            DriftType::Spc => {
                let profile = serde_json::from_str(&profile)?;
                Ok(DriftProfile::Spc(profile))
            }
            DriftType::Psi => {
                let profile = serde_json::from_str(&profile)?;
                Ok(DriftProfile::Psi(profile))
            }
            DriftType::Custom => {
                let profile = serde_json::from_str(&profile)?;
                Ok(DriftProfile::Custom(profile))
            }
            DriftType::GenAI => {
                let profile = serde_json::from_str(&profile)?;
                Ok(DriftProfile::GenAI(profile))
            }
        }
    }

    /// Get the base arguments for a drift profile
    pub fn get_base_args(&self) -> ProfileArgs {
        match self {
            DriftProfile::Spc(profile) => profile.get_base_args(),
            DriftProfile::Psi(profile) => profile.get_base_args(),
            DriftProfile::Custom(profile) => profile.get_base_args(),
            DriftProfile::GenAI(profile) => profile.get_base_args(),
        }
    }

    pub fn to_value(&self) -> serde_json::Value {
        match self {
            DriftProfile::Spc(profile) => profile.to_value(),
            DriftProfile::Psi(profile) => profile.to_value(),
            DriftProfile::Custom(profile) => profile.to_value(),
            DriftProfile::GenAI(profile) => profile.to_value(),
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
    pub fn from_value(body: serde_json::Value) -> Result<Self, ProfileError> {
        let drift_type = body["config"]["drift_type"].as_str().unwrap();

        let drift_type = DriftType::from_str(drift_type)?;

        match drift_type {
            DriftType::Spc => {
                let profile = serde_json::from_value(body)?;
                Ok(DriftProfile::Spc(profile))
            }
            DriftType::Psi => {
                let profile = serde_json::from_value(body)?;
                Ok(DriftProfile::Psi(profile))
            }
            DriftType::Custom => {
                let profile = serde_json::from_value(body)?;
                Ok(DriftProfile::Custom(profile))
            }
            DriftType::LLM => {
                let profile = serde_json::from_value(body)?;
                Ok(DriftProfile::GenAI(profile))
            }
        }
    }

    pub fn from_python(
        drift_type: DriftType,
        profile: &Bound<'_, PyAny>,
    ) -> Result<Self, ProfileError> {
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
            DriftType::LLM => {
                let profile = profile.extract::<LLMDriftProfile>()?;
                Ok(DriftProfile::GenAI(profile))
            }
        }
    }

    pub fn get_spc_profile(&self) -> Result<&SpcDriftProfile, ProfileError> {
        match self {
            DriftProfile::Spc(profile) => Ok(profile),
            _ => Err(ProfileError::InvalidDriftTypeError),
        }
    }

    pub fn get_psi_profile(&self) -> Result<&PsiDriftProfile, ProfileError> {
        match self {
            DriftProfile::Psi(profile) => Ok(profile),
            _ => Err(ProfileError::InvalidDriftTypeError),
        }
    }

    pub fn get_llm_profile(&self) -> Result<&LLMDriftProfile, ProfileError> {
        match self {
            DriftProfile::GenAI(profile) => Ok(profile),
            _ => Err(ProfileError::InvalidDriftTypeError),
        }
    }

    pub fn drift_type(&self) -> DriftType {
        match self {
            DriftProfile::Spc(_) => DriftType::Spc,
            DriftProfile::Psi(_) => DriftType::Psi,
            DriftProfile::Custom(_) => DriftType::Custom,
            DriftProfile::GenAI(_) => DriftType::LLM,
        }
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::DriftProfile.to_str(),
        )?)
    }

    pub fn load_from_json(path: PathBuf) -> Result<Self, ProfileError> {
        let file = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&file)?)
    }

    /// load a profile into the DriftProfile enum from path
    ///
    /// # Arguments
    /// * `path` - Path to the profile
    ///
    /// # Returns
    /// * `Result<Self>` - Result of DriftProfile
    pub fn from_profile_path(path: PathBuf) -> Result<Self, ProfileError> {
        let profile = std::fs::read_to_string(&path)?;
        let profile_value: Value = serde_json::from_str(&profile).unwrap();
        DriftProfile::from_value(profile_value)
    }

    pub fn version(&self) -> Option<String> {
        match self {
            DriftProfile::Spc(profile) => Some(profile.config.version.clone()),
            DriftProfile::Psi(profile) => Some(profile.config.version.clone()),
            DriftProfile::Custom(profile) => Some(profile.config.version.clone()),
            DriftProfile::GenAI(profile) => Some(profile.config.version.clone()),
        }
    }

    pub fn identifier(&self) -> String {
        match self {
            DriftProfile::Spc(profile) => {
                format!(
                    "{}/{}/v{}/spc",
                    profile.config.space, profile.config.name, profile.config.version
                )
            }
            DriftProfile::Psi(profile) => {
                format!(
                    "{}/{}/v{}/psi",
                    profile.config.space, profile.config.name, profile.config.version
                )
            }
            DriftProfile::Custom(profile) => {
                format!(
                    "{}/{}/v{}/custom",
                    profile.config.space, profile.config.name, profile.config.version
                )
            }
            DriftProfile::GenAI(profile) => {
                format!(
                    "{}/{}/v{}/llm",
                    profile.config.space, profile.config.name, profile.config.version
                )
            }
        }
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
