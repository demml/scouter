use ndarray_stats::MaybeNan;
use num_traits::{Float, FromPrimitive, Num};
use numpy::ndarray::ArrayView2;
use numpy::ndarray::{concatenate, Axis};
use numpy::PyArray2;
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
use scouter::core::error::ProfilerError;
use scouter::core::error::ScouterError;
use scouter::core::profile::num_profiler::NumProfiler;
use scouter::core::profile::string_profiler::StringProfiler;
use scouter::core::profile::types::{DataProfile, FeatureProfile};
use scouter::core::stats::compute_feature_correlations;
use scouter::core::utils::create_feature_map;
use std::collections::BTreeMap;
use std::collections::HashMap;

#[pyclass]
pub struct ScouterProfiler {
    num_profiler: NumProfiler,
    string_profiler: StringProfiler,
}

impl ScouterProfiler {
    fn process_string_and_num_array<F>(
        &mut self,
        compute_correlations: bool,
        numeric_array: ArrayView2<F>,
        string_array: Vec<Vec<String>>,
        numeric_features: Vec<String>,
        string_features: Vec<String>,
        bin_size: Option<usize>,
    ) -> Result<DataProfile, ProfilerError>
    where
        F: Float
            + MaybeNan
            + FromPrimitive
            + std::fmt::Display
            + Sync
            + Send
            + Num
            + Clone
            + std::fmt::Debug
            + 'static
            + std::convert::Into<f64>,
        F: Into<f64>,
        <F as MaybeNan>::NotNan: Ord,
        f64: From<F>,

        <F as MaybeNan>::NotNan: Clone,
    {
        // run  StringProfiler in separate thread

        let string_profiles = self
            .string_profiler
            .create_string_profile(&string_array, &string_features)
            .map_err(|e| {
                ProfilerError::StringProfileError(format!("Failed to create string profile: {}", e))
            })?;

        let num_profiles = self
            .num_profiler
            .compute_stats(&numeric_features, &numeric_array, &bin_size.unwrap_or(20))
            .map_err(|e| {
                ProfilerError::ComputeError(format!("Failed to create feature data profile: {}", e))
            })?;

        let correlations: Option<HashMap<String, HashMap<String, f64>>> = if compute_correlations {
            let converted_array = self
                .string_profiler
                .convert_string_vec_to_num_array(&string_array, &string_features)
                .map_err(|e| {
                    ProfilerError::ConversionError(format!(
                        "Failed to convert string array to numeric array: {}",
                        e
                    ))
                })?;

            // convert all values to F
            let converted_array = converted_array.mapv(|x| F::from_f64(x).unwrap());

            // combine numeric_array and converted_array
            let concatenated_array = {
                let numeric_array_view = numeric_array.view();
                let converted_array_view = converted_array.view();
                concatenate(Axis(1), &[numeric_array_view, converted_array_view]).map_err(|e| {
                    ProfilerError::ArrayError(format!(
                        "Failed to concatenate numeric and converted arrays: {}",
                        e
                    ))
                })?
            };

            // merge numeric and string features
            let mut features = numeric_features.clone();
            features.append(&mut string_features.clone());

            let correlations = compute_feature_correlations(&concatenated_array.view(), &features);
            Some(correlations)
        } else {
            None
        };

        let mut features: BTreeMap<String, FeatureProfile> = string_profiles
            .iter()
            .map(|profile| (profile.id.clone(), profile.clone()))
            .collect();

        let num_features: BTreeMap<String, FeatureProfile> = num_profiles
            .iter()
            .map(|profile| (profile.id.clone(), profile.clone()))
            .collect();

        features.extend(num_features);

        Ok(DataProfile {
            features,
            correlations,
        })
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl ScouterProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            num_profiler: NumProfiler::default(),
            string_profiler: StringProfiler::default(),
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        compute_correlations: bool,
        numeric_array: Option<PyReadonlyArray2<f32>>,
        string_array: Option<Vec<Vec<String>>>,
        numeric_features: Option<Vec<String>>,
        string_features: Option<Vec<String>>,
        bin_size: Option<usize>,
    ) -> PyResult<DataProfile> {
        if string_features.is_some() && string_array.is_some() && numeric_array.is_none() {
            let profile = self
                .string_profiler
                .process_string_array::<f32>(
                    string_array.unwrap(),
                    string_features.unwrap(),
                    compute_correlations,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;
            Ok(profile)
        } else if string_array.is_none() && numeric_array.is_some() && numeric_features.is_some() {
            let profile = self
                .num_profiler
                .process_num_array(
                    compute_correlations,
                    &numeric_array.unwrap().as_array(),
                    numeric_features.unwrap(),
                    bin_size,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;

            Ok(profile)
        } else {
            let profile = self
                .process_string_and_num_array(
                    compute_correlations,
                    numeric_array.unwrap().as_array(),
                    string_array.unwrap(),
                    numeric_features.unwrap(),
                    string_features.unwrap(),
                    bin_size,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;

            Ok(profile)
        }
    }

    pub fn create_data_profile_f64(
        &mut self,
        compute_correlations: bool,
        numeric_array: Option<PyReadonlyArray2<f64>>,
        string_array: Option<Vec<Vec<String>>>,
        numeric_features: Option<Vec<String>>,
        string_features: Option<Vec<String>>,
        bin_size: Option<usize>,
    ) -> PyResult<DataProfile> {
        if string_features.is_some() && string_array.is_some() && numeric_array.is_none() {
            let profile = self
                .string_profiler
                .process_string_array::<f32>(
                    string_array.unwrap(),
                    string_features.unwrap(),
                    compute_correlations,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;
            Ok(profile)
        } else if string_array.is_none() && numeric_array.is_some() && numeric_features.is_some() {
            let profile = self
                .num_profiler
                .process_num_array(
                    compute_correlations,
                    &numeric_array.unwrap().as_array(),
                    numeric_features.unwrap(),
                    bin_size,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;

            Ok(profile)
        } else {
            let profile = self
                .process_string_and_num_array(
                    compute_correlations,
                    numeric_array.unwrap().as_array(),
                    string_array.unwrap(),
                    numeric_features.unwrap(),
                    string_features.unwrap(),
                    bin_size,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;

            Ok(profile)
        }
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
        let feature_map = match create_feature_map(&features, &array) {
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
