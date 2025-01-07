use ndarray_stats::MaybeNan;
use num_traits::{Float, FromPrimitive, Num};
use numpy::ndarray::ArrayView2;
use numpy::ndarray::{concatenate, Axis};
use numpy::PyArray2;
use numpy::PyReadonlyArray2;
use numpy::ToPyArray;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter_drift::{
    psi::PsiMonitor,
    spc::{generate_alerts, SpcDriftMap, SpcMonitor},
    CategoricalFeatureHelpers,
};
use scouter_error::{ProfilerError, ScouterError};
use scouter_profile::{
    compute_feature_correlations, DataProfile, FeatureProfile, NumProfiler, StringProfiler,
};
use scouter_types::{
    create_feature_map,
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::{SpcAlertRule, SpcDriftConfig, SpcDriftProfile, SpcFeatureAlerts},
    ServerRecords,
    DataType,
};
use std::collections::BTreeMap;
use std::collections::HashMap;
use tracing::info;

#[pyclass]
pub struct ScouterProfiler {
    num_profiler: NumProfiler,
    string_profiler: StringProfiler,
}

#[pymethods]
impl ScouterProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            num_profiler: NumProfiler::default(),
            string_profiler: StringProfiler::default(),
        }
    }

    #[pyo3(signature = (data, data_type, bin_size=None, compute_correlations=None))]
    pub fn create_data_profile(&self,  data: &Bound<'_, PyAny>, data_type: &DataType, bin_size: Option<usize>, compute_correlations: Option<bool>) -> PyResult<()> {

        info!("Creating data profile");



        let bin_ize = bin_size.unwrap_or(20);
        let compute_correlations = compute_correlations.unwrap_or(false);

        let array = match data_type {
            DataType::Pandas => {
                let array = data.extract::<PyReadonlyArray2<f32>>()?;
                self.create_data_profile_f32(compute_correlations, Some(array), None, None, None, Some(bin_size))?;
            }
           
            _ => {
                return Err(PyValueError::new_err("Invalid data type"));
            }
        };

        Ok(())
    }
       
}

impl ScouterProfiler {

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
        <F as MaybeNan>::NotNan: Ord,
        f64: From<F>,
        <F as MaybeNan>::NotNan: Clone,
    {
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

        let correlations: Option<HashMap<String, HashMap<String, f32>>> = if compute_correlations {
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
            let converted_array = converted_array.mapv(|x| F::from(x).unwrap());

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
            .map(|profile| {
                let mut profile = profile.clone();

                if let Some(correlations) = correlations.as_ref() {
                    let correlation = correlations.get(&profile.id);
                    if let Some(correlation) = correlation {
                        profile.add_correlations(correlation.clone());
                    }
                }

                (profile.id.clone(), profile)
            })
            .collect();

        let num_features: BTreeMap<String, FeatureProfile> = num_profiles
            .iter()
            .map(|profile| {
                let mut profile = profile.clone();

                if let Some(correlations) = correlations.as_ref() {
                    let correlation = correlations.get(&profile.id);
                    if let Some(correlation) = correlation {
                        profile.add_correlations(correlation.clone());
                    }
                }

                (profile.id.clone(), profile)
            })
            .collect();

        features.extend(num_features);

        Ok(DataProfile { features })
    }
}


impl ScouterProfiler {
   
   
}