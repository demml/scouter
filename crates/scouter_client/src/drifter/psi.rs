use crate::data_utils::{convert_array_type, ConvertedData};
use num_traits::{Float, FromPrimitive};
use numpy::PyReadonlyArray2;
use scouter_drift::{psi::PsiMonitor, CategoricalFeatureHelpers};
use scouter_error::ScouterError;
use scouter_types::psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile};
use std::collections::HashMap;

pub struct PsiDrifter {
    monitor: PsiMonitor,
}

impl PsiDrifter {
    pub fn new() -> Self {
        let monitor = PsiMonitor::new();
        PsiDrifter { monitor }
    }

    pub fn compute_drift<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_profile: PsiDriftProfile,
    ) -> Result<PsiDriftMap, ScouterError>
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
        F: numpy::Element,
    {
        let drift_map = self
            .monitor
            .compute_drift(&features, &array.as_array(), &drift_profile)?;
        Ok(drift_map)
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: PsiDriftConfig,
    ) -> Result<PsiDriftProfile, ScouterError> {
        let feature_map = match self.monitor.create_feature_map(&features, &array) {
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
                        "Failed to create 2D monitor profile".to_string(),
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
                        "Failed to create 2D monitor profile".to_string(),
                    ));
                }
            };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: PsiDriftConfig,
    ) -> Result<PsiDriftProfile, ScouterError>
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
        F: numpy::Element,
    {
        let array = array.as_array();

        let profile = self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)?;

        Ok(profile)
    }

    pub fn create_drift_profile<'py>(
        &mut self,
        data: ConvertedData<'py>,
        config: PsiDriftConfig,
    ) -> Result<PsiDriftProfile, ScouterError> {
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

        Ok(PsiDriftProfile::new(features, final_config, None))
    }
}
