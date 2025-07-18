use crate::error::DriftError;
use crate::utils::CategoricalFeatureHelpers;
use itertools::Itertools;
use ndarray::prelude::*;
use ndarray::Axis;
use num_traits::{Float, FromPrimitive};
use rayon::prelude::*;
use scouter_types::psi::{
    Bin, BinType, PsiDriftConfig, PsiDriftMap, PsiDriftProfile, PsiFeatureDriftProfile,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct PsiMonitor {}

impl CategoricalFeatureHelpers for PsiMonitor {}

impl PsiMonitor {
    pub fn new() -> Self {
        PsiMonitor {}
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

    fn create_categorical_bins<F>(&self, column_vector: &ArrayView<F, Ix1>) -> Vec<Bin>
    where
        F: Float + FromPrimitive + Default + Sync,
        F: Into<f64>,
    {
        let vector_len = column_vector.len() as f64;
        let mut counts: HashMap<usize, usize> = HashMap::new();

        for &value in column_vector.iter() {
            let key = Into::<f64>::into(value) as usize;
            *counts.entry(key).or_insert(0) += 1;
        }

        counts
            .into_par_iter()
            .map(|(id, count)| Bin {
                id,
                lower_limit: None,
                upper_limit: None,
                proportion: (count as f64) / vector_len,
            })
            .collect()
    }

    fn create_numeric_bins<F>(
        &self,
        column_vector: &ArrayView1<F>,
        drift_config: &PsiDriftConfig,
    ) -> Result<Vec<Bin>, DriftError>
    where
        F: Float + FromPrimitive + Default + Sync,
        F: Into<f64>,
    {
        let edges = drift_config
            .binning_strategy
            .compute_edges(column_vector)
            .map_err(|e| DriftError::BinningError(e.to_string()))?;

        let bins: Vec<Bin> = (0..=edges.len())
            .into_par_iter()
            .map(|decile| {
                let lower = if decile == 0 {
                    F::neg_infinity()
                } else {
                    edges[decile - 1]
                };
                let upper = if decile == edges.len() {
                    F::infinity()
                } else {
                    edges[decile]
                };
                let bin_count = self.compute_bin_count(column_vector, &lower.into(), &upper.into());
                Bin {
                    id: decile + 1,
                    lower_limit: Some(lower.into()),
                    upper_limit: Some(upper.into()),
                    proportion: (bin_count as f64) / (column_vector.len() as f64),
                }
            })
            .collect();
        Ok(bins)
    }

    fn create_bins<F>(
        &self,
        feature_name: &String,
        column_vector: &ArrayView<F, Ix1>,
        drift_config: &PsiDriftConfig,
    ) -> Result<(Vec<Bin>, BinType), DriftError>
    where
        F: Float + FromPrimitive + Default + Sync,
        F: Into<f64>,
    {
        match &drift_config.categorical_features {
            Some(features) if features.contains(feature_name) => {
                // Process as categorical
                Ok((
                    self.create_categorical_bins(column_vector),
                    BinType::Category,
                ))
            }
            _ => {
                // Process as continuous
                Ok((
                    self.create_numeric_bins(column_vector, drift_config)?,
                    BinType::Numeric,
                ))
            }
        }
    }

    fn create_psi_feature_drift_profile<F>(
        &self,
        feature_name: String,
        column_vector: &ArrayView<F, Ix1>,
        drift_config: &PsiDriftConfig,
    ) -> Result<PsiFeatureDriftProfile, DriftError>
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
    {
        let (bins, bin_type) = self.create_bins(&feature_name, column_vector, drift_config)?;

        Ok(PsiFeatureDriftProfile {
            id: feature_name,
            bins,
            timestamp: chrono::Utc::now(),
            bin_type,
        })
    }

    fn clean_column_vector<F>(column_vector: &ArrayView<F, Ix1>) -> Array<F, Ix1>
    where
        F: Float,
    {
        Array1::from(
            column_vector
                .iter()
                .filter(|&&x| x.is_finite())
                .cloned()
                .collect::<Vec<F>>(),
        )
    }

    pub fn create_2d_drift_profile<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        drift_config: &PsiDriftConfig,
    ) -> Result<PsiDriftProfile, DriftError>
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
                let clean_column_vector = Self::clean_column_vector(&column_vector);

                if clean_column_vector.is_empty() {
                    return Err(DriftError::EmptyArrayError(
                        format!("PSI drift profile creation failure, unable to create psi feature drift profile for {feature_name}")
                    ));
                }

                self.create_psi_feature_drift_profile(
                    feature_name.to_string(),
                    &clean_column_vector.view(),
                    drift_config,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        profile_vector
            .into_iter()
            .zip(features)
            .for_each(|(profile, feature_name)| {
                psi_feature_drift_profiles.insert(feature_name.clone(), profile);
            });

        Ok(PsiDriftProfile::new(
            psi_feature_drift_profiles,
            drift_config.clone(),
            None,
        ))
    }

    fn compute_psi_proportion_pairs<F>(
        &self,
        column_vector: &ArrayView<F, Ix1>,
        bin: &Bin,
        feature_is_categorical: bool,
    ) -> Result<(f64, f64), DriftError>
    where
        F: Float + FromPrimitive,
        F: Into<f64>,
    {
        if feature_is_categorical {
            let bin_count = column_vector
                .iter()
                .filter(|&&value| value.into() == bin.id as f64)
                .count();
            return Ok((
                bin.proportion,
                (bin_count as f64) / (column_vector.len() as f64),
            ));
        }

        let bin_count = self.compute_bin_count(
            column_vector,
            &bin.lower_limit.unwrap(),
            &bin.upper_limit.unwrap(),
        );

        Ok((
            bin.proportion,
            (bin_count as f64) / (column_vector.len() as f64),
        ))
    }

    pub fn compute_psi(proportion_pairs: &[(f64, f64)]) -> f64 {
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
        feature_drift_profile: &PsiFeatureDriftProfile,
        feature_is_categorical: bool,
    ) -> Result<f64, DriftError>
    where
        F: Float + Sync + FromPrimitive,
        F: Into<f64>,
    {
        let bins = &feature_drift_profile.bins;
        let feature_proportions: Vec<(f64, f64)> = bins
            .into_par_iter()
            .map(|bin| {
                self.compute_psi_proportion_pairs(column_vector, bin, feature_is_categorical)
            })
            .collect::<Result<Vec<(f64, f64)>, DriftError>>()?;

        Ok(PsiMonitor::compute_psi(&feature_proportions))
    }

    fn check_features<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        drift_profile: &PsiDriftProfile,
    ) -> Result<(), DriftError>
    where
        F: Float + Sync + FromPrimitive,
        F: Into<f64>,
    {
        assert_eq!(
            features.len(),
            array.shape()[1],
            "Feature count must match column count."
        );

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

                    return Err(DriftError::RunTimeError(
                        format!(
                            "Feature mismatch, feature '{feature_name}' not found. Available features in the drift profile: {available_keys}"
                        ),
                    ));
                }
                Ok(())
            })
    }

    pub fn compute_drift<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        drift_profile: &PsiDriftProfile,
    ) -> Result<PsiDriftMap, DriftError>
    where
        F: Float + Sync + FromPrimitive,
        F: Into<f64>,
    {
        self.check_features(features, array, drift_profile)?;

        let drift_values: Vec<_> = array
            .axis_iter(Axis(1))
            .zip(features)
            .collect_vec()
            .into_par_iter()
            .map(|(column_vector, feature_name)| {
                let feature_is_categorical = drift_profile
                    .config
                    .categorical_features
                    .as_ref()
                    .is_some_and(|features| features.contains(feature_name));
                self.compute_feature_drift(
                    &column_vector,
                    drift_profile.features.get(feature_name).unwrap(),
                    feature_is_categorical,
                )
            })
            .collect::<Result<Vec<f64>, DriftError>>()?;

        let mut psi_drift_features = HashMap::new();

        drift_values
            .into_iter()
            .zip(features)
            .for_each(|(drift_value, feature_name)| {
                psi_drift_features.insert(feature_name.clone(), drift_value);
            });

        Ok(PsiDriftMap {
            features: psi_drift_features,
            name: drift_profile.config.name.clone(),
            space: drift_profile.config.space.clone(),
            version: drift_profile.config.version.clone(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    #[test]
    fn test_check_features_all_exist() {
        let psi_monitor = PsiMonitor::default();

        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let profile = psi_monitor
            .create_2d_drift_profile(&features, &array.view(), &PsiDriftConfig::default())
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let result = psi_monitor.check_features(&features, &array.view(), &profile);

        // Assert that the result is Ok
        assert!(result.is_ok());
    }

    #[test]
    fn test_compute_psi_basic() {
        let proportions = vec![(0.3, 0.2), (0.4, 0.4), (0.3, 0.4)];

        let result = PsiMonitor::compute_psi(&proportions);

        // Manually compute expected PSI for this case
        let expected_psi = (0.3 - 0.2) * (0.3 / 0.2).ln()
            + (0.4 - 0.4) * (0.4 / 0.4).ln()
            + (0.3 - 0.4) * (0.3 / 0.4).ln();

        assert!((result - expected_psi).abs() < 1e-6);
    }

    #[test]
    fn test_compute_bin_count() {
        let psi_monitor = PsiMonitor::default();

        let data = Array1::from_vec(vec![1.0, 2.5, 3.7, 5.0, 6.3, 8.1]);

        let lower_threshold = 2.0;
        let upper_threshold = 6.0;

        let result =
            psi_monitor.compute_bin_count(&data.view(), &lower_threshold, &upper_threshold);

        // Check that it counts the correct number of elements within the bin
        // In this case, 2.5, 3.7, and 5.0 should be counted (3 elements)
        assert_eq!(result, 3);
    }

    #[test]
    fn test_compute_psi_proportion_pairs_categorical() {
        let psi_monitor = PsiMonitor::default();

        let cat_vector = Array::from_vec(vec![0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);

        let cat_zero_bin = Bin {
            id: 0,
            lower_limit: None,
            upper_limit: None,
            proportion: 0.4,
        };

        let (_, prod_proportion) = psi_monitor
            .compute_psi_proportion_pairs(&cat_vector.view(), &cat_zero_bin, true)
            .unwrap();

        let expected_prod_proportion = 0.5;

        assert!(
            (prod_proportion - expected_prod_proportion).abs() < 1e-9,
            "prod_proportion was expected to be 50%"
        );
    }

    #[test]
    fn test_compute_psi_proportion_pairs_non_categorical() {
        let psi_monitor = PsiMonitor::default();

        let vector = Array::from_vec(vec![
            12.0, 11.0, 10.0, 1.0, 10.0, 21.0, 19.0, 12.0, 12.0, 23.0,
        ]);

        let bin = Bin {
            id: 1,
            lower_limit: Some(0.0),
            upper_limit: Some(11.0),
            proportion: 0.4,
        };

        let (_, prod_proportion) = psi_monitor
            .compute_psi_proportion_pairs(&vector.view(), &bin, false)
            .unwrap();

        let expected_prod_proportion = 0.4;

        assert!(
            (prod_proportion - expected_prod_proportion).abs() < 1e-9,
            "prod_proportion was expected to be 40%"
        );
    }

    #[test]
    fn test_create_bins_non_categorical() {
        let psi_monitor = PsiMonitor::default();

        let non_categorical_data = Array::from_vec(vec![
            120.0, 1.0, 33.0, 71.0, 15.0, 59.0, 8.0, 62.0, 4.0, 21.0, 10.0, 2.0, 344.0, 437.0,
            53.0, 39.0, 83.0, 6.0, 4.30, 2.0,
        ]);

        let result = psi_monitor.create_numeric_bins(
            &ArrayView::from(&non_categorical_data),
            &PsiDriftConfig::default(),
        );

        assert!(result.is_ok());
        let bins = result.unwrap();
        assert_eq!(bins.len(), 10);
    }

    #[test]
    fn test_create_bins_categorical() {
        // let psi_monitor = PsiMonitor::default();

        let categorical_data = Array::from_vec(vec![
            1.0, 1.0, 2.0, 3.0, 2.0, 3.0, 2.0, 1.0, 2.0, 1.0, 1.0, 2.0, 3.0, 3.0, 2.0, 3.0, 1.0,
            1.0,
        ]);

        println!("{:?}", categorical_data);

        // let bins = psi_monitor.create_categorical_bins(&ArrayView::from(&categorical_data));
        // assert_eq!(bins.len(), 3);
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

        let monitor = PsiMonitor::default();
        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &PsiDriftConfig::default())
            .unwrap();

        assert_eq!(profile.features.len(), 3);
    }

    #[test]
    fn test_compute_drift() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        // cast array to f32
        let array = array.mapv(|x| x as f32);

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = PsiMonitor::default();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &PsiDriftConfig::default())
            .unwrap();

        let drift_map = monitor
            .compute_drift(&features, &array.view(), &profile)
            .unwrap();

        assert_eq!(drift_map.features.len(), 3);

        // assert that the drift values are all 0.0
        drift_map
            .features
            .values()
            .for_each(|value| assert!(*value == 0.0));

        // create new array that has drifted values
        let mut new_array = Array::random((1030, 3), Uniform::new(0., 10.)).mapv(|x| x as f32);
        new_array.slice_mut(s![.., 0]).mapv_inplace(|x| x + 0.01);

        let new_drift_map = monitor
            .compute_drift(&features, &new_array.view(), &profile)
            .unwrap();

        // assert that the drift values are all greater than 0.0
        new_drift_map
            .features
            .values()
            .for_each(|value| assert!(*value > 0.0));
    }
}
