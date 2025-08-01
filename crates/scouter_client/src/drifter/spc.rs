use crate::data_utils::{convert_array_type, ConvertedData};
use ndarray::Axis;
use ndarray::{concatenate, Array2};
use num_traits::{Float, FromPrimitive, Num};
use numpy::PyReadonlyArray2;
use scouter_drift::error::DriftError;
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
use std::sync::Arc;
use std::sync::RwLock;
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
    ) -> Result<Array2<f32>, DriftError> {
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
    ) -> Result<Array2<f64>, DriftError> {
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
    ) -> Result<SpcFeatureAlerts, DriftError> {
        let drift_array = drift_array.as_array();

        generate_alerts(&drift_array, &features, &alert_rule)
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        drift_config: Arc<RwLock<SpcDriftConfig>>,
    ) -> Result<SpcDriftProfile, DriftError> {
        let feature_map = match create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(e) => {
                return Err(e.into());
            }
        };

        drift_config
            .write()
            .unwrap()
            .update_feature_map(feature_map.clone());

        let array = self
            .monitor
            .convert_strings_to_ndarray_f32(&features, &array, &feature_map)?;

        self.monitor.create_2d_drift_profile(
            &features,
            &array.view(),
            &drift_config.read().unwrap(),
        )
    }

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: &SpcDriftConfig,
    ) -> Result<SpcDriftProfile, DriftError>
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
            .create_2d_drift_profile(&features, &array, drift_config)?;

        Ok(profile)
    }

    pub fn compute_drift(
        &mut self,
        data: ConvertedData<'_>,
        drift_profile: SpcDriftProfile,
    ) -> Result<SpcDriftMap, DriftError> {
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
        config: Arc<RwLock<SpcDriftConfig>>,
    ) -> Result<SpcDriftProfile, DriftError> {
        let (num_features, num_array, dtype, string_features, string_array) = data;

        let mut features = HashMap::new();

        if let Some(string_array) = string_array {
            let profile =
                self.create_string_drift_profile(string_array, string_features, config.clone())?;
            features.extend(profile.features);
        }

        if let Some(num_array) = num_array {
            let dtype = dtype.unwrap();
            let drift_profile = {
                let read_config = config.read().unwrap();
                if dtype == "float64" {
                    let array = convert_array_type::<f64>(num_array, &dtype)?;
                    self.create_numeric_drift_profile(array, num_features, &read_config)?
                } else {
                    let array = convert_array_type::<f32>(num_array, &dtype)?;
                    self.create_numeric_drift_profile(array, num_features, &read_config)?
                }
            };
            features.extend(drift_profile.features);
        }

        {
            let mut write_config = config.write().unwrap();

            if write_config.alert_config.features_to_monitor.is_empty() {
                write_config.alert_config.features_to_monitor = features.keys().cloned().collect();
            }

            // Validate features_to_monitor
            if let Some(missing_feature) = write_config
                .alert_config
                .features_to_monitor
                .iter()
                .find(|&key| !features.contains_key(key))
            {
                return Err(DriftError::FeatureToMonitorMissingError(
                    missing_feature.to_string(),
                ));
            }
        }

        let config_clone = config.read().unwrap().clone();

        Ok(SpcDriftProfile::new(features, config_clone))
    }
}
