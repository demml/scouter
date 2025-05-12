use crate::data_utils::{convert_array_type, ConvertedData};
use ndarray::Axis;
use ndarray::{concatenate, Array2};
use num_traits::{Float, FromPrimitive, Num};
use numpy::PyReadonlyArray2;
use scouter_drift::error::PyDriftError;
use scouter_drift::{
    spc::{generate_alerts, SpcDriftMap, SpcMonitor},
    CategoricalFeatureHelpers,
};
use scouter_types::{
    create_feature_map,
    spc::{SpcAlertRule, SpcDriftConfig, SpcDriftProfile, SpcFeatureAlerts},
};
use std::collections::HashMap;
use std::fmt::Debug;

#[derive(Default)]
pub struct SpcDrifter {
    monitor: SpcMonitor,
}

impl SpcDrifter {
    pub fn new() -> Self {
        let monitor = SpcMonitor::new();
        SpcDrifter { monitor }
    }

    pub fn convert_strings_to_numpy_f32(
        &mut self,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> Result<Array2<f32>, PyDriftError> {
        let array = self.monitor.convert_strings_to_ndarray_f32(
            &features,
            &array,
            &drift_profile.config.feature_map,
        )?;

        Ok(array)
    }

    pub fn convert_strings_to_numpy_f64(
        &mut self,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> Result<Array2<f64>, PyDriftError> {
        let array = self.monitor.convert_strings_to_ndarray_f64(
            &features,
            &array,
            &drift_profile.config.feature_map,
        )?;

        Ok(array)
    }

    pub fn generate_alerts(
        &mut self,
        drift_array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        alert_rule: SpcAlertRule,
    ) -> Result<SpcFeatureAlerts, PyDriftError> {
        let drift_array = drift_array.as_array();

        Ok(generate_alerts(&drift_array, &features, &alert_rule)?)
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, PyDriftError> {
        let feature_map = match create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(e) => {
                return Err(e.into());
            }
        };

        drift_config.update_feature_map(feature_map.clone());

        let array =
            match self
                .monitor
                .convert_strings_to_ndarray_f32(&features, &array, &feature_map)
            {
                Ok(array) => array,
                Err(e) => {
                    return Err(e.into());
                }
            };

        let profile =
            match self
                .monitor
                .create_2d_drift_profile(&features, &array.view(), &drift_config)
            {
                Ok(profile) => profile,
                Err(e) => {
                    return Err(e.into());
                }
            };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, PyDriftError>
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

    pub fn compute_drift(
        &mut self,
        data: ConvertedData<'_>,
        drift_profile: SpcDriftProfile,
    ) -> Result<SpcDriftMap, PyDriftError> {
        let (num_features, num_array, dtype, string_features, string_array) = data;
        let dtype = dtype.unwrap_or("float32".to_string());

        let mut features = num_features.clone();
        features.extend(string_features.clone());

        if let Some(string_array) = string_array {
            if dtype == "float64" {
                let string_array = self.convert_strings_to_numpy_f64(
                    string_features,
                    string_array,
                    drift_profile.clone(),
                )?;

                if num_array.is_some() {
                    let array = convert_array_type::<f64>(num_array.unwrap(), &dtype)?;
                    let concatenated =
                        concatenate(Axis(1), &[array.as_array(), string_array.view()])?;
                    Ok(self.monitor.compute_drift(
                        &features,
                        &concatenated.view(),
                        &drift_profile,
                    )?)
                } else {
                    Ok(self.monitor.compute_drift(
                        &features,
                        &string_array.view(),
                        &drift_profile,
                    )?)
                }
            } else {
                let string_array = self.convert_strings_to_numpy_f32(
                    string_features,
                    string_array,
                    drift_profile.clone(),
                )?;

                if num_array.is_some() {
                    let array = convert_array_type::<f32>(num_array.unwrap(), &dtype)?;
                    let concatenated =
                        concatenate(Axis(1), &[array.as_array(), string_array.view()])?;
                    Ok(self.monitor.compute_drift(
                        &features,
                        &concatenated.view(),
                        &drift_profile,
                    )?)
                } else {
                    Ok(self.monitor.compute_drift(
                        &features,
                        &string_array.view(),
                        &drift_profile,
                    )?)
                }
            }
        } else if dtype == "float64" {
            let array = convert_array_type::<f64>(num_array.unwrap(), &dtype)?;
            Ok(self
                .monitor
                .compute_drift(&num_features, &array.as_array(), &drift_profile)?)
        } else {
            let array = convert_array_type::<f32>(num_array.unwrap(), &dtype)?;
            Ok(self
                .monitor
                .compute_drift(&num_features, &array.as_array(), &drift_profile)?)
        }
    }

    pub fn create_drift_profile(
        &mut self,
        data: ConvertedData<'_>,
        config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, PyDriftError> {
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
