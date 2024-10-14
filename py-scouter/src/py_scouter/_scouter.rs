use numpy::{IntoPyArray, PyArray1, PyArray2};
use numpy::PyReadonlyArray2;
use numpy::ToPyArray;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter::core::drift::base::ServerRecords;
use scouter::core::drift::spc::alert::generate_alerts;
use scouter::core::drift::spc::monitor::SpcMonitor;
use scouter::core::drift::spc::types::{
    SpcAlertRule, SpcDriftConfig, SpcDriftMap, SpcDriftProfile, SpcFeatureAlerts,
};
use scouter::core::error::ScouterError;
use scouter::core::profile::num_profiler::NumProfiler;
use scouter::core::profile::string_profiler::StringProfiler;
use scouter::core::profile::types::{DataProfile, FeatureProfile};
use std::collections::BTreeMap;
use numpy::ndarray::{Array1, Array2};
use scouter::core::drift::psi::monitor::PsiMonitor;

fn create_string_profile(
    string_array: Vec<Vec<String>>,
    string_features: Vec<String>,
) -> Result<Vec<FeatureProfile>, ScouterError> {
    let string_profiler = StringProfiler::new();
    let string_profile = string_profiler
        .compute_2d_stats(&string_array, &string_features)
        .map_err(|_e| ScouterError::StringProfileError(_e.to_string()))?;

    Ok(string_profile)
}

#[pyclass]
pub struct ScouterProfiler {
    num_profiler: NumProfiler,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl ScouterProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            num_profiler: NumProfiler::default(),
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        numeric_array: Option<PyReadonlyArray2<f32>>,
        string_array: Option<Vec<Vec<String>>>,
        numeric_features: Option<Vec<String>>,
        string_features: Option<Vec<String>>,
        bin_size: Option<usize>,
    ) -> PyResult<DataProfile> {
        let mut profiles = vec![];

        // process string features
        if string_features.is_some() && string_array.is_some() {
            let string_profile =
                create_string_profile(string_array.unwrap(), string_features.unwrap());

            let string_profile = match string_profile {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err(
                        "Failed to create feature data profile",
                    ));
                }
            };

            profiles.extend(string_profile);

            // run  StringProfiler in separate thread
        }

        // process numeric features
        if numeric_features.is_some() && numeric_array.is_some() {
            let numeric_features = numeric_features.unwrap();
            let num_profiles = match self.num_profiler.compute_stats(
                &numeric_features,
                &numeric_array.unwrap().as_array(),
                &bin_size.unwrap_or(20),
            ) {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err(
                        "Failed to create feature data profile",
                    ));
                }
            };

            profiles.extend(num_profiles);
        }

        let mut features = BTreeMap::new();
        for profile in &profiles {
            features.insert(profile.id.clone(), profile.clone());
        }

        Ok(DataProfile { features })
    }

    pub fn create_data_profile_f64(
        &mut self,
        numeric_array: Option<PyReadonlyArray2<f64>>,
        string_array: Option<Vec<Vec<String>>>,
        numeric_features: Option<Vec<String>>,
        string_features: Option<Vec<String>>,
        bin_size: Option<usize>,
    ) -> PyResult<DataProfile> {
        let mut profiles = vec![];

        // process string features
        if string_features.is_some() && string_array.is_some() {
            let string_profile =
                create_string_profile(string_array.unwrap(), string_features.unwrap());

            let string_profile = match string_profile {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err(
                        "Failed to create feature data profile",
                    ));
                }
            };

            profiles.extend(string_profile);

            // run  StringProfiler in separate thread
        }

        // process numeric features
        if numeric_features.is_some() && numeric_array.is_some() {
            let numeric_features = numeric_features.unwrap();
            let num_profiles = match self.num_profiler.compute_stats(
                &numeric_features,
                &numeric_array.unwrap().as_array(),
                &bin_size.unwrap_or(20),
            ) {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err(
                        "Failed to create feature data profile",
                    ));
                }
            };

            profiles.extend(num_profiles);
        }

        let mut features = BTreeMap::new();
        for profile in &profiles {
            features.insert(profile.id.clone(), profile.clone());
        }

        Ok(DataProfile { features })
    }
}

#[pyclass]
pub struct SpcDrifter {
    monitor: SpcMonitor,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcDrifter {
    #[new]
    pub fn new() -> Self {
        //maybe create different drifters based on type of monitoring?
        Self {
            monitor: SpcMonitor::new(),
        }
    }

    pub fn convert_strings_to_numpy_f32<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<pyo3::Bound<'py, PyArray2<f32>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f32(
            &features,
            &array,
            &drift_profile
                .config
                .feature_map
                .ok_or(ScouterError::MissingFeatureMapError)
                .unwrap(),
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray_bound(py))
    }

    pub fn convert_strings_to_numpy_f64<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<pyo3::Bound<'py, PyArray2<f64>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f64(
            &features,
            &array,
            &drift_profile
                .config
                .feature_map
                .ok_or(ScouterError::MissingFeatureMapError)
                .unwrap(),
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray_bound(py))
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let feature_map = match self.monitor.create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(_e) => {
                let msg = format!("Failed to create feature map: {}", _e);
                return Err(PyValueError::new_err(msg));
            }
        };

        drift_config.update_feature_map(feature_map.clone());

        let array =
            match self
                .monitor
                .convert_strings_to_ndarray_f32(&features, &array, &feature_map)
            {
                Ok(array) => array,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
                }
            };

        for i in &array{
            println!("{i:?}");
        }

        let profile =
            match self
                .monitor
                .create_2d_drift_profile(&features, &array.view(), &drift_config)
            {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
                }
            };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn compute_drift_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap> {
        let drift_map =
            match self
                .monitor
                .compute_drift(&features, &array.as_array(), &drift_profile)
            {
                Ok(drift_map) => drift_map,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to compute drift"));
                }
            };

        Ok(drift_map)
    }

    pub fn compute_drift_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap> {
        let drift_map =
            match self
                .monitor
                .compute_drift(&features, &array.as_array(), &drift_profile)
            {
                Ok(drift_map) => drift_map,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to compute drift"));
                }
            };

        Ok(drift_map)
    }

    pub fn generate_alerts(
        &mut self,
        drift_array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        alert_rule: SpcAlertRule,
    ) -> PyResult<SpcFeatureAlerts> {
        let drift_array = drift_array.as_array();

        let alerts = match generate_alerts(&drift_array, &features, &alert_rule) {
            Ok(alerts) => alerts,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to generate alerts"));
            }
        };

        Ok(alerts)
    }

    pub fn sample_data_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<ServerRecords> {
        let array = array.as_array();

        let records = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(records)
    }

    pub fn sample_data_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<ServerRecords> {
        let array = array.as_array();

        let records = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(records)
    }
}

#[pyclass]
pub struct PsiDrifter {
    monitor: PsiMonitor,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl PsiDrifter {
    #[new]
    pub fn new() -> Self {
        Self {
            monitor: PsiMonitor::new(),
        }
    }
    pub fn return_dummy_data<'py>(
        &mut self,
        py: Python<'py>,
        feature_names: Vec<String>,
        features_array: Vec<Vec<String>>,
    ) -> PyResult<(Vec<String>, Bound<'py, PyArray2<f32>>)> {

        // Create a simple Vec<String> for dummy string data
        let dummy_strings = vec![
            "feature1".to_string(),
            "feature2".to_string(),
            "feature3".to_string(),
        ];

        // Create dummy 2D array of floats with 100 rows and 3 columns
        let dummy_numeric: Array2<f32> = Array2::from_shape_vec(
            (100, 3),  // 100 rows, 3 columns
            (1..=300).map(|x| x as f32).collect(), // Fill with values from 1.0 to 300.0
        ).unwrap();

        // Convert the numeric array to a NumPy array bound to Python
        let py_numeric_array = dummy_numeric.to_pyarray_bound(py);

        // Return both the list of strings and the NumPy array
        Ok((dummy_strings, py_numeric_array))
    }
}