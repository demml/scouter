use crate::data_utils::{convert_array_type, DataConverterEnum};
use ndarray_stats::MaybeNan;
use num_traits::{Float, FromPrimitive, Num};
use numpy::ndarray::ArrayView2;
use numpy::ndarray::{concatenate, Axis};
use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter_error::{ProfilerError, PyScouterError};
use scouter_profile::{
    compute_feature_correlations, DataProfile, FeatureProfile, NumProfiler, StringProfiler,
};
use scouter_types::DataType;
use std::collections::BTreeMap;
use std::collections::HashMap;
use tracing::info;

#[pyclass]
pub struct DataProfiler {
    num_profiler: NumProfiler,
    string_profiler: StringProfiler,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl DataProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            num_profiler: NumProfiler::default(),
            string_profiler: StringProfiler::default(),
        }
    }

    #[pyo3(signature = (data, data_type=None, bin_size=20, compute_correlations=false))]
    pub fn create_data_profile(
        &mut self,
        data: &Bound<'_, PyAny>,
        data_type: Option<&DataType>,
        bin_size: Option<usize>,
        compute_correlations: Option<bool>,
    ) -> PyResult<DataProfile> {
        info!("Creating data profile");

        let bin_size = bin_size.unwrap_or(20);
        let compute_correlations = compute_correlations.unwrap_or(false);

        // if data_type is None, try to infer it from the class name
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{}.{}", module, name);

                &DataType::from_module_name(&full_class_name)?
            }
        };

        let (num_features, num_array, dtype, string_features, string_vec) =
            DataConverterEnum::convert_data(data_type, data)?;

        // if num_features is not empty, check dtype. If dtype == "float64", process as f64, else process as f32
        if let Some(dtype) = dtype {
            if dtype == "float64" {
                let read_array = convert_array_type::<f64>(num_array.unwrap(), &dtype)?;

                return self
                    .create_data_profile_f64(
                        compute_correlations,
                        bin_size,
                        num_features,
                        Some(read_array),
                        string_features,
                        string_vec,
                    )
                    .map_err(|e| PyScouterError::new_err(e.to_string()));
            } else {
                let read_array = convert_array_type::<f32>(num_array.unwrap(), &dtype)?;
                return self
                    .create_data_profile_f32(
                        compute_correlations,
                        bin_size,
                        num_features,
                        Some(read_array),
                        string_features,
                        string_vec,
                    )
                    .map_err(|e| PyScouterError::new_err(e.to_string()));
            }
        }

        self.create_data_profile_f32(
            compute_correlations,
            bin_size,
            num_features,
            None,
            string_features,
            string_vec,
        )
        .map_err(|e| PyScouterError::new_err(e.to_string()))
    }
}

impl DataProfiler {
    pub fn create_data_profile_f32(
        &mut self,
        compute_correlations: bool,
        bin_size: usize,
        numeric_features: Vec<String>,
        numeric_array: Option<PyReadonlyArray2<f32>>,
        string_features: Vec<String>,
        string_array: Option<Vec<Vec<String>>>,
    ) -> PyResult<DataProfile> {
        if !string_features.is_empty() && string_array.is_some() && numeric_array.is_none() {
            let profile = self
                .string_profiler
                .process_string_array::<f32>(
                    string_array.unwrap(),
                    string_features,
                    compute_correlations,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;
            Ok(profile)
        } else if string_array.is_none() && numeric_array.is_some() && !numeric_features.is_empty()
        {
            let profile = self
                .num_profiler
                .process_num_array(
                    compute_correlations,
                    &numeric_array.unwrap().as_array(),
                    numeric_features,
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
                    numeric_features,
                    string_features,
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
        bin_size: usize,
        numeric_features: Vec<String>,
        numeric_array: Option<PyReadonlyArray2<f64>>,
        string_features: Vec<String>,
        string_array: Option<Vec<Vec<String>>>,
    ) -> PyResult<DataProfile> {
        if !string_features.is_empty() && string_array.is_some() && numeric_array.is_none() {
            let profile = self
                .string_profiler
                .process_string_array::<f32>(
                    string_array.unwrap(),
                    string_features,
                    compute_correlations,
                )
                .map_err(|e| {
                    PyValueError::new_err(format!("Failed to create feature data profile: {}", e))
                })?;
            Ok(profile)
        } else if string_array.is_none() && numeric_array.is_some() && !numeric_features.is_empty()
        {
            let profile = self
                .num_profiler
                .process_num_array(
                    compute_correlations,
                    &numeric_array.unwrap().as_array(),
                    numeric_features,
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
                    numeric_features,
                    string_features,
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
        bin_size: usize,
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
            .compute_stats(&numeric_features, &numeric_array, &bin_size)
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
