use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter_drift::{
    spc::{generate_alerts, SpcDriftMap, SpcMonitor},
    CategoricalFeatureHelpers,
};

use crate::data_utils::{convert_array_type, ConvertedData};
use num_traits::{Float, FromPrimitive, Num};
use scouter_error::ScouterError;
use scouter_types::{
    create_feature_map,
    spc::{SpcAlertRule, SpcDriftConfig, SpcDriftProfile, SpcFeatureAlerts},
};
use std::collections::HashMap;
use std::fmt::Debug;

pub struct SpcDrifter {
    monitor: SpcMonitor,
}

impl SpcDrifter {
    pub fn new() -> Self {
        let monitor = SpcMonitor::new();
        SpcDrifter { monitor }
    }

    pub fn generate_alerts(
        &mut self,
        drift_array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        alert_rule: SpcAlertRule,
    ) -> Result<SpcFeatureAlerts, ScouterError> {
        let drift_array = drift_array.as_array();

        Ok(generate_alerts(&drift_array, &features, &alert_rule)?)
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, ScouterError> {
        let feature_map = match create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(_e) => {
                let msg = format!("Failed to create feature map: {}", _e);
                return Err(ScouterError::Error(msg));
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
                    return Err(ScouterError::Error(
                        "Failed to create 2D drift profile".to_string(),
                    ));
                }
            };

        let profile =
            match self
                .monitor
                .create_2d_drift_profile(&features, &array.view(), &drift_config)
            {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(ScouterError::Error(
                        "Failed to create 2D drift profile".to_string(),
                    ));
                }
            };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, ScouterError>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand
            + numpy::Element,
        F: Into<f64>,
    {
        let array = array.as_array();

        let profile = self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)?;

        Ok(profile)
    }

    pub fn compute_drift<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand
            + numpy::Element,
        F: Into<f64>,
    {
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

    pub fn create_drift_profile<'py>(
        &mut self,
        data: ConvertedData<'py>,
        config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, ScouterError> {
        let (num_features, num_array, dtype, string_features, string_array) = data;

        let mut features = HashMap::new();
        let cfg = config.clone();
        let mut final_config = cfg.clone();

        if let Some(string_array) = string_array {
            let profile = self.create_string_drift_profile(
                string_array,
                string_features,
                final_config.clone(),
            )?;
            final_config.feature_map = profile.config.feature_map.clone();
            features.extend(profile.features);
        }

        if let Some(num_array) = num_array {
            let dtype = dtype.unwrap();
            let drift_profile = if dtype == "float64" {
                let array = convert_array_type::<f64>(num_array, &dtype)?;
                self.create_numeric_drift_profile(array, num_features, final_config.clone())?
            } else {
                let array = convert_array_type::<f32>(num_array, &dtype)?;
                self.create_numeric_drift_profile(array, num_features, final_config.clone())?
            };
            features.extend(drift_profile.features);
        }

        Ok(SpcDriftProfile::new(features, final_config, None))
    }
}
