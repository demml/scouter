use crate::core::error::MonitorError;
use crate::utils::types::{Bin, DriftConfig, PSIDriftMap, PSIDriftProfile, PSIFeatureDriftProfile};
use itertools::Itertools;
use ndarray::prelude::*;
use ndarray::Axis;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt::Debug;

pub struct PSIMonitor {}

impl PSIMonitor {
    pub fn new() -> Self {
        PSIMonitor {}
    }

    fn compute_1d_array_mean<F>(&self, array: &ArrayView<F, Ix1>) -> Result<f64, MonitorError>
    where
        F: Float + FromPrimitive,
        F: Into<f64>,
    {
        Ok(array
            .mean()
            .ok_or(MonitorError::ComputeError(
                "Failed to compute mean".to_string(),
            ))?
            .into())
    }

    fn compute_bin_count<F>(
        &self,
        array: &ArrayView<F, Ix1>,
        lower_threshold: &f64,
        upper_threshold: &f64,
    ) -> usize
    where
        F: Float + FromPrimitive,
        F: Into<f64>,
    {
        array
            .iter()
            .filter(|&&value| value.into() > *lower_threshold && value.into() <= *upper_threshold)
            .count()
    }

    fn data_are_binary<F>(&self, column_vector: &ArrayView<F, Ix1>) -> bool
    where
        F: Float,
        F: Into<f64>,
    {
        column_vector
            .iter()
            .all(|&value| value.into() == 0.0 || value.into() == 1.0)
    }

    fn compute_deciles<F>(&self, column_vector: &ArrayView<F, Ix1>) -> Result<[F; 9], MonitorError>
    where
        F: Float + Default,
        F: Into<f64>,
    {
        if column_vector.len() < 10 {
            return Err(MonitorError::ComputeError(
                "At least 10 values needed to compute deciles".to_string(),
            ));
        }

        let sorted_column_vector = column_vector
            .iter()
            .sorted_by(|a, b| a.partial_cmp(b).unwrap()) // Use partial_cmp and unwrap since we assume no NaNs
            .cloned()
            .collect_vec();

        let n = sorted_column_vector.len();
        let mut deciles: [F; 9] = Default::default();

        for i in 1..=9 {
            let index = ((i as f32 * (n as f32 - 1.0)) / 10.0).floor() as usize;
            deciles[i - 1] = sorted_column_vector[index];
        }
        Ok(deciles)
    }

    fn create_bins<F>(&self, column_vector: &ArrayView<F, Ix1>) -> Result<Vec<Bin>, MonitorError>
    where
        F: Float + FromPrimitive + Default + Sync,
        F: Into<f64>,
    {
        if self.data_are_binary(column_vector) {
            let column_vector_mean = self.compute_1d_array_mean(column_vector)?;

            Ok(vec![
                Bin {
                    id: 1,
                    lower_limit: 0.0,
                    upper_limit: None,
                    proportion: 1.0 - column_vector_mean,
                },
                Bin {
                    id: 2,
                    lower_limit: 1.0,
                    upper_limit: None,
                    proportion: column_vector_mean,
                },
            ])
        } else {
            let deciles = self.compute_deciles(column_vector).map_err(|err| {
                MonitorError::ComputeError(format!("Failed to compute deciles: {}", err))
            })?;

            let bins: Vec<Bin> = (0..=deciles.len())
                .into_par_iter()
                .map(|decile| {
                    let lower = if decile == 0 {
                        F::neg_infinity()
                    } else {
                        deciles[decile - 1]
                    };
                    let upper = if decile == deciles.len() {
                        F::infinity()
                    } else {
                        deciles[decile]
                    };
                    let bin_count =
                        self.compute_bin_count(column_vector, &lower.into(), &upper.into());
                    Bin {
                        id: decile + 1,
                        lower_limit: lower.into(),
                        upper_limit: Some(upper.into()),
                        proportion: (bin_count as f64) / (column_vector.len() as f64),
                    }
                })
                .collect();
            Ok(bins)
        }
    }

    fn create_psi_feature_drift_profile<F>(
        &self,
        feature_name: String,
        column_vector: &ArrayView<F, Ix1>,
    ) -> Result<PSIFeatureDriftProfile, MonitorError>
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
    {
        let bins = self
            .create_bins(column_vector)
            .map_err(|err| MonitorError::CreateError(format!("Failed to create bins: {}", err)))?;

        Ok(PSIFeatureDriftProfile {
            id: feature_name,
            bins,
            timestamp: chrono::Utc::now().naive_utc(),
        })
    }

    // Create a 2D monitor profile
    //
    // # Arguments
    //
    // * `features` - A vector of feature names
    // * `array` - A 2D array of f64 values
    //
    // # Returns
    //
    // A monitor profile
    pub fn create_2d_drift_profile<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        drift_config: &DriftConfig,
    ) -> Result<PSIDriftProfile, MonitorError>
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
    {
        let mut psi_feature_drift_profiles = HashMap::new();

        // Ensure that the number of features matches the number of columns in the array
        assert_eq!(
            features.len(),
            array.shape()[1],
            "Feature count must match column count."
        );

        let profile_vector = array
            .axis_iter(Axis(1))
            .zip(features)
            .collect_vec()
            .into_par_iter()
            .map(|(column_vector, feature_name)| {
                self.create_psi_feature_drift_profile(feature_name.to_string(), &column_vector)
            })
            .collect::<Result<Vec<_>, _>>()?;

        profile_vector
            .into_iter()
            .zip(features)
            .for_each(|(profile, feature_name)| {
                psi_feature_drift_profiles.insert(feature_name.clone(), profile);
            });

        Ok(PSIDriftProfile::new(
            psi_feature_drift_profiles,
            drift_config.clone(),
            None,
        ))
    }

    fn compute_psi_proportion_pairs<F>(
        &self,
        column_vector: &ArrayView<F, Ix1>,
        bin: &Bin,
    ) -> Result<(f64, f64), MonitorError>
    where
        F: Float + FromPrimitive,
        F: Into<f64>,
    {
        if self.data_are_binary(column_vector) {
            let column_vector_mean = self.compute_1d_array_mean(column_vector)?;
            if bin.id == 1 {
                return Ok((bin.proportion, 1.0 - column_vector_mean));
            }
            return Ok((bin.proportion, column_vector_mean));
        }
        let bin_count =
            self.compute_bin_count(column_vector, &bin.lower_limit, &bin.upper_limit.unwrap());

        Ok((
            bin.proportion,
            (bin_count as f64) / (column_vector.len() as f64),
        ))
    }

    fn compute_psi(&self, proportion_pairs: &Vec<(f64, f64)>) -> f64 {
        let epsilon = 1e-10;
        proportion_pairs
            .iter()
            .map(|(p, q)| {
                let p_adj = p + epsilon;
                let q_adj = q + epsilon;
                (p_adj - q_adj) * (p_adj / q_adj).ln()
            })
            .sum()
    }

    fn compute_feature_drift<F>(
        &self,
        column_vector: &ArrayView<F, Ix1>,
        features: &PSIFeatureDriftProfile,
    ) -> Result<f64, MonitorError>
    where
        F: Float + Sync + FromPrimitive,
        F: Into<f64>,
    {
        let bins = &features.bins;
        let feature_proportions: Vec<(f64, f64)> = bins
            .into_par_iter()
            .map(|bin| self.compute_psi_proportion_pairs(column_vector, bin))
            .collect::<Result<Vec<(f64, f64)>, MonitorError>>()?;
        Ok(self.compute_psi(&feature_proportions))
    }

    fn check_features(
        &self,
        features: &[String],
        drift_profile: &PSIDriftProfile,
    ) -> Result<(), MonitorError> {
        features
            .iter()
            .try_for_each(|feature_name| {
                if !drift_profile.features.contains_key(feature_name) {
                    // Collect all the keys from the drift profile into a comma-separated string
                    let available_keys = drift_profile
                        .features
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");

                    return Err(MonitorError::ComputeError(
                        format!(
                            "Feature mismatch, feature '{}' not found. Available features in the drift profile: {}",
                            feature_name, available_keys
                        ),
                    ));
                }
                Ok(())
            })
    }

    pub fn compute_model_drift<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        drift_profile: &PSIDriftProfile,
    ) -> Result<PSIDriftMap, MonitorError>
    where
        F: Float + Sync + FromPrimitive,
        F: Into<f64>,
    {
        assert_eq!(
            features.len(),
            array.shape()[1],
            "Feature count must match column count."
        );

        self.check_features(features, drift_profile)?;

        let drift_values: Vec<_> = array
            .axis_iter(Axis(1))
            .zip(features)
            .collect_vec()
            .into_par_iter()
            .map(|(column_vector, feature_name)| {
                self.compute_feature_drift(
                    &column_vector,
                    &drift_profile.features.get(feature_name).unwrap(),
                )
            })
            .collect::<Result<Vec<f64>, MonitorError>>()?;

        let mut psi_drift_features = HashMap::new();

        drift_values
            .into_iter()
            .zip(features)
            .for_each(|(drift_value, feature_name)| {
                psi_drift_features.insert(feature_name.clone(), drift_value);
            });

        Ok(PSIDriftMap {
            features: psi_drift_features,
            name: drift_profile.config.name.clone(),
            repository: drift_profile.config.repository.clone(),
            version: drift_profile.config.version.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use crate::utils::types::AlertConfig;

    use super::*;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    // #[test]
    // fn test_compute_drift() {
    //     let psi_monitor = PSIMonitor::new();
    //
    //     let array = Array::random((1030, 3), Uniform::new(0., 10.));
    //
    //     let features = vec![
    //         "feature_1".to_string(),
    //         "feature_2".to_string(),
    //         "feature_3".to_string(),
    //     ];
    //     let config = DriftConfig::new(
    //         Some("name".to_string()),
    //         Some("repo".to_string()),
    //         None,
    //         None,
    //         None,
    //         None,
    //         None,
    //         None,
    //         None,
    //     );
    //
    //     let profile = psi_monitor
    //         .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
    //         .unwrap();
    //     assert_eq!(profile.features.len(), 3);
    //
    //     // change first 100 rows to 100 at index 1
    //     let mut array = array.to_owned();
    //     array.slice_mut(s![0..200, 1]).fill(100.0);
    //
    //     let drift_map = psi_monitor
    //         .compute_model_drift(&features, &array.view(), &profile)
    //         .unwrap();
    // }

    #[test]
    fn test_data_are_binary_with_binary_vector() {
        let psi_monitor = PSIMonitor::new();

        let binary_vector = Array::from_vec(vec![0.0, 1.0, 1.0, 0.0, 0.0]);
        let column_view = binary_vector.view();

        let result = psi_monitor.data_are_binary(&column_view);

        assert!(result, "Expected the binary vector to return true");
    }

    #[test]
    fn test_data_are_binary_with_non_binary_vector() {
        let psi_monitor = PSIMonitor::new();

        let non_binary_vector = Array::from_vec(vec![0.0, 2.0, 1.0, 3.0, 0.0]);
        let column_view = non_binary_vector.view();

        let result = psi_monitor.data_are_binary(&column_view);

        assert!(!result, "Expected the non-binary vector to return false");
    }

    #[test]
    fn test_compute_deciles_with_unsorted_input() {
        let psi_monitor = PSIMonitor::new();

        let unsorted_vector = Array::from_vec(vec![
            120.0, 1.0, 33.0, 71.0, 15.0, 59.0, 8.0, 62.0, 4.0, 21.0, 10.0, 2.0, 344.0, 437.0,
            53.0, 39.0, 83.0, 6.0, 4.30, 2.0,
        ]);
        let column_view = unsorted_vector.view();

        let result = psi_monitor.compute_deciles(&column_view);

        let expected_deciles: [f64; 9] = [2.0, 4.0, 6.0, 10.0, 21.0, 39.0, 59.0, 71.0, 120.0];

        assert_eq!(
            result.unwrap().as_ref(),
            expected_deciles.as_ref(),
            "Deciles computed incorrectly for unsorted input"
        );
    }

    #[test]
    fn test_create_bins_binary() {
        let psi_monitor = PSIMonitor::new();

        let binary_data = Array::from_vec(vec![0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

        let result = psi_monitor.create_bins(&ArrayView::from(&binary_data)); // Replace `your_struct_instance` with the actual instance

        assert!(result.is_ok());
        let bins = result.unwrap();
        assert_eq!(bins.len(), 2);
    }

    #[test]
    fn test_create_bins_non_binary() {
        let psi_monitor = PSIMonitor::new();

        let non_binary_data = Array::from_vec(vec![
            120.0, 1.0, 33.0, 71.0, 15.0, 59.0, 8.0, 62.0, 4.0, 21.0, 10.0, 2.0, 344.0, 437.0,
            53.0, 39.0, 83.0, 6.0, 4.30, 2.0,
        ]);

        let result = psi_monitor.create_bins(&ArrayView::from(&non_binary_data));

        assert!(result.is_ok());
        let bins = result.unwrap();
        assert_eq!(bins.len(), 10);
    }

    #[test]
    fn test_create_2d_drift_profile() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        // cast array to f32
        let array = array.mapv(|x| x as f32);

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let alert_config = AlertConfig::default();
        let monitor = PSIMonitor::new();
        let config = DriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
            Some(alert_config),
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        assert_eq!(profile.features.len(), 3);
    }
}
