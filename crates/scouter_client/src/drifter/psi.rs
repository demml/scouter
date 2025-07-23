use crate::data_utils::{convert_array_type, ConvertedData};
use ndarray::{concatenate, Array2, Axis};
use num_traits::{Float, FromPrimitive};
use numpy::PyReadonlyArray2;
use scouter_drift::error::DriftError;
use scouter_drift::{psi::PsiMonitor, CategoricalFeatureHelpers};
use scouter_types::psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::instrument;
#[derive(Default)]
pub struct PsiDrifter {
    monitor: PsiMonitor,
}

impl PsiDrifter {
    pub fn new() -> Self {
        let monitor = PsiMonitor::new();
        PsiDrifter { monitor }
    }

    pub fn convert_strings_to_numpy_f32(
        &mut self,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: PsiDriftProfile,
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
        drift_profile: PsiDriftProfile,
    ) -> Result<Array2<f64>, DriftError> {
        let array = self.monitor.convert_strings_to_ndarray_f64(
            &features,
            &array,
            &drift_profile.config.feature_map,
        )?;

        Ok(array)
    }

    #[instrument(skip_all)]
    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        drift_config: Arc<RwLock<PsiDriftConfig>>,
    ) -> Result<PsiDriftProfile, DriftError> {
        let feature_map = self.monitor.create_feature_map(&features, &array)?;

        drift_config
            .write()
            .unwrap()
            .update_feature_map(feature_map.clone());

        let array = self
            .monitor
            .convert_strings_to_ndarray_f32(&features, &array, &feature_map)?;

        let profile = self.monitor.create_2d_drift_profile(
            &features,
            &array.view(),
            &drift_config.read().unwrap(),
        )?;

        Ok(profile)
    }

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: &PsiDriftConfig,
    ) -> Result<PsiDriftProfile, DriftError>
    where
        F: Float + Sync + FromPrimitive + Default + PartialOrd,
        F: Into<f64>,
        F: numpy::Element,
    {
        let array = array.as_array();

        let profile = self
            .monitor
            .create_2d_drift_profile(&features, &array, drift_config)?;

        Ok(profile)
    }

    pub fn create_drift_profile(
        &mut self,
        data: ConvertedData<'_>,
        config: Arc<RwLock<PsiDriftConfig>>,
    ) -> Result<PsiDriftProfile, DriftError> {
        let (num_features, num_array, dtype, string_features, string_array) = data;

        // Validate categorical_features
        if let Some(categorical_features) = config.read().unwrap().categorical_features.as_ref() {
            // fail if the specified categorical features are not in the num_features or string_features
            if let Some(missing_feature) = categorical_features
                .iter()
                .find(|&key| !num_features.contains(key) && !string_features.contains(key))
            {
                return Err(DriftError::CategoricalFeatureMissingError(
                    missing_feature.to_string(),
                ));
            }
        }

        let mut features = HashMap::new();

        if let Some(string_array) = string_array {
            let profile =
                self.create_string_drift_profile(string_array, string_features, config.clone())?;
            features.extend(profile.features);
        }

        let read_config = config.read().unwrap();
        if let Some(num_array) = num_array {
            let dtype = dtype.unwrap();
            let drift_profile = if dtype == "float64" {
                let array = convert_array_type::<f64>(num_array, &dtype)?;
                self.create_numeric_drift_profile(array, num_features, &read_config)?
            } else {
                let array = convert_array_type::<f32>(num_array, &dtype)?;
                self.create_numeric_drift_profile(array, num_features, &read_config)?
            };
            features.extend(drift_profile.features);
        }

        // if config.features_to_monitor is empty, set it to all features
        if read_config.alert_config.features_to_monitor.is_empty() {
            config.write().unwrap().alert_config.features_to_monitor =
                features.keys().cloned().collect();
        }

        // Validate features_to_monitor
        if let Some(missing_feature) = read_config
            .alert_config
            .features_to_monitor
            .iter()
            .find(|&key| !features.contains_key(key))
        {
            return Err(DriftError::FeatureToMonitorMissingError(
                missing_feature.to_string(),
            ));
        }

        Ok(PsiDriftProfile::new(
            features,
            config.read().unwrap().clone(),
            None,
        ))
    }

    pub fn compute_drift(
        &mut self,
        data: ConvertedData<'_>,
        drift_profile: PsiDriftProfile,
    ) -> Result<PsiDriftMap, DriftError> {
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
}
