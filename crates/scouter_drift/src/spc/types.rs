#![allow(clippy::useless_conversion)]
use crate::error::DriftError;
use core::fmt::Debug;
use ndarray::Array;
use ndarray::Array2;
use numpy::{IntoPyArray, PyArray2};
use pyo3::prelude::*;
use scouter_types::error::UtilError;

use scouter_types::{FileName, ProfileFuncs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Python class for a feature drift
///
/// # Arguments
///
/// * `samples` - A vector of samples
/// * `drift` - A vector of drift values
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcFeatureDrift {
    #[pyo3(get)]
    pub samples: Vec<f64>,

    #[pyo3(get)]
    pub drift: Vec<f64>,
}

impl SpcFeatureDrift {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

/// Python class for a Drift map of features with calculated drift
///
/// # Arguments
///
/// * `features` - A hashmap of feature names and their drift
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftMap {
    #[pyo3(get)]
    pub features: HashMap<String, SpcFeatureDrift>,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub version: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcDriftMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<SpcDriftMap, UtilError> {
        // deserialize the string to a struct
        Ok(serde_json::from_str(&json_string)?)
    }

    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, UtilError> {
        ProfileFuncs::save_to_json(self, path, FileName::SpcDriftMap.to_str())
    }

    #[allow(clippy::type_complexity)]
    pub fn to_numpy<'py>(
        &self,
        py: Python<'py>,
    ) -> Result<
        (
            Bound<'py, PyArray2<f64>>,
            Bound<'py, PyArray2<f64>>,
            Vec<String>,
        ),
        DriftError,
    > {
        let (drift_array, sample_array, features) = self.to_array()?;

        Ok((
            drift_array.into_pyarray(py).to_owned(),
            sample_array.into_pyarray(py).to_owned(),
            features,
        ))
    }
}

type ArrayReturn = (Array2<f64>, Array2<f64>, Vec<String>);

impl SpcDriftMap {
    pub fn new(space: String, name: String, version: String) -> Self {
        Self {
            features: HashMap::new(),
            name,
            space,
            version,
        }
    }

    pub fn to_array(&self) -> Result<ArrayReturn, DriftError> {
        let columns = self.features.len();
        let rows = self.features.values().next().unwrap().samples.len();

        // create empty array
        let mut drift_array = Array2::<f64>::zeros((rows, columns));
        let mut sample_array = Array2::<f64>::zeros((rows, columns));
        let mut features = Vec::new();

        // iterate over the features and insert the drift values
        for (i, (feature, drift)) in self.features.iter().enumerate() {
            features.push(feature.clone());
            drift_array
                .column_mut(i)
                .assign(&Array::from(drift.drift.clone()));
            sample_array
                .column_mut(i)
                .assign(&Array::from(drift.samples.clone()));
        }

        Ok((drift_array, sample_array, features))
    }

    pub fn add_feature(&mut self, feature: String, drift: SpcFeatureDrift) {
        self.features.insert(feature, drift);
    }
}
// Drift config to use when calculating drift on a new sample of data
