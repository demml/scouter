use crate::utils::types::DriftServerRecord;
use crate::utils::types::{
    DriftConfig, DriftMap, DriftProfile, FeatureDrift, FeatureDriftProfile, FeatureMap,
};
use anyhow::Ok;
use anyhow::{Context, Result};
use indicatif::ProgressBar;
use ndarray::prelude::*;
use ndarray::Axis;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Debug;
pub struct Monitor {}

impl Monitor {
    pub fn new() -> Self {
        Monitor {}
    }

    /// Compute c4 for control limits
    ///
    /// # Arguments
    ///
    /// * `number` - The sample size
    ///
    /// # Returns
    ///
    /// The c4 value
    fn compute_c4(&self, number: usize) -> f32 {
        //c4 is asymptotically equivalent to (4n-4)/(4n-3)
        let n = number as f32;
        let left = 4.0 * n - 4.0;
        let right = 4.0 * n - 3.0;
        left / right
    }

    /// Set the sample size based on the shape of the array
    ///
    /// # Arguments
    ///
    /// * `shape` - The shape of the array
    ///
    /// # Returns
    ///
    /// The sample size
    fn set_sample_size(&self, shape: usize) -> usize {
        if shape < 1000 {
            25
        } else if (1000..10000).contains(&shape) {
            100
        } else if (10000..100000).contains(&shape) {
            1000
        } else if (100000..1000000).contains(&shape) {
            10000
        } else if shape >= 1000000 {
            100000
        } else {
            25
        }
    }

    /// Compute the mean for a 2D array
    ///
    /// # Arguments
    ///
    /// * `x` - A 2D array of f64 values
    ///
    /// # Returns
    ///
    /// A 1D array of f64 values
    pub fn compute_array_mean<F>(&self, x: &ArrayView2<F>) -> Result<Array1<F>, anyhow::Error>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let means = x
            .mean_axis(Axis(0))
            .with_context(|| "Failed to compute mean")?;

        Ok(means)
    }

    // Computes control limits for a 2D array of data
    // Control limits are calculated as per NIST standards
    // https://www.itl.nist.gov/div898/handbook/pmc/section3/pmc32.htm
    //
    // # Arguments
    //
    // * `sample_size` - The sample size
    // * `sample_data` - A 2D array of f64 values
    // * `num_features` - The number of features
    // * `features` - A vector of feature names
    // * `monitor_config` - A monitor config
    fn compute_control_limits<F>(
        &self,
        sample_size: usize,
        sample_data: &ArrayView2<F>,
        num_features: usize,
        features: &[String],
        monitor_config: &DriftConfig,
    ) -> Result<DriftProfile, anyhow::Error>
    where
        F: FromPrimitive + Num + Clone + Float + Debug + Sync + Send + ndarray::ScalarOperand,

        F: Into<f64>,
    {
        let c4 = self.compute_c4(sample_size);
        let sample_mean = self
            .compute_array_mean(sample_data)
            .with_context(|| "Failed to compute mean")?;

        let means = sample_mean.slice(s![0..num_features]);
        let stdev = sample_mean.slice(s![num_features..]);
        // calculate control limit arrays

        let base = &stdev / F::from(c4).unwrap();
        let one_sigma = &base * F::from(1.0).unwrap();
        let two_sigma = &base * F::from(2.0).unwrap();
        let three_sigma = &base * F::from(3.0).unwrap();

        // calculate control limits for each zone
        let one_lcl = &means - &one_sigma;
        let one_ucl = &means + &one_sigma;

        let two_lcl = &means - &two_sigma;
        let two_ucl = &means + &two_sigma;

        let three_lcl = &means - &three_sigma;
        let three_ucl = &means + &three_sigma;
        let center = &means;

        // create monitor profile
        let mut feat_profile = BTreeMap::new();

        for (i, feature) in features.iter().enumerate() {
            feat_profile.insert(
                feature.to_string(),
                FeatureDriftProfile {
                    id: feature.to_string(),
                    center: center[i].into(),
                    one_ucl: one_ucl[i].into(),
                    one_lcl: one_lcl[i].into(),
                    two_ucl: two_ucl[i].into(),
                    two_lcl: two_lcl[i].into(),
                    three_ucl: three_ucl[i].into(),
                    three_lcl: three_lcl[i].into(),
                    timestamp: chrono::Utc::now().naive_utc(),
                },
            );
        }

        Ok(DriftProfile {
            features: feat_profile,
            config: monitor_config.clone(),
        })
    }

    /// Create a 2D monitor profile
    ///
    /// # Arguments
    ///
    /// * `features` - A vector of feature names
    /// * `array` - A 2D array of f64 values
    ///
    /// # Returns
    ///
    /// A monitor profile
    pub fn create_2d_drift_profile<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
        monitor_config: &DriftConfig,
    ) -> Result<DriftProfile, anyhow::Error>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let shape = array.shape()[0];
        let num_features = features.len();
        let sample_size = self.set_sample_size(shape);

        let nbr_chunks = shape / sample_size;
        let pb = ProgressBar::new(nbr_chunks as u64);

        // iterate through each feature
        let sample_vec = array
            .axis_chunks_iter(Axis(0), sample_size)
            .into_par_iter()
            .map(|x| {
                let mean = x
                    .mean_axis(Axis(0))
                    .with_context(|| "Failed to compute mean")
                    .unwrap();
                let stddev = x.std_axis(
                    Axis(0),
                    F::from(1.0)
                        .with_context(|| "Failed to create f64 for stddev")
                        .unwrap(),
                );

                // append stddev to mean
                let combined = ndarray::concatenate![Axis(0), mean, stddev];
                //mean.remove_axis(Axis(1));
                pb.inc(1);
                combined.to_vec()
            })
            .collect::<Vec<_>>();

        // reshape vec to 2D array
        let sample_data =
            Array::from_shape_vec((sample_vec.len(), features.len() * 2), sample_vec.concat())
                .with_context(|| "Failed to create 2D array")?;

        let drift_profile = self
            .compute_control_limits(
                sample_size,
                &sample_data.view(),
                num_features,
                features,
                monitor_config,
            )
            .with_context(|| "Failed to compute control limits")?;

        Ok(drift_profile)
    }

    // Samples data for drift detection
    //
    // # Arguments
    //
    // * `array` - A 2D array of f64 values
    // * `sample_size` - The sample size
    // * `columns` - The number of columns
    //
    // # Returns
    // A 2D array of f64 values
    fn _sample_data<F>(
        &self,
        array: &ArrayView2<F>,
        sample_size: usize,
        columns: usize,
    ) -> Result<Array2<f64>, anyhow::Error>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let sample_vec: Vec<Vec<f64>> = array
            .axis_chunks_iter(Axis(0), sample_size)
            .into_par_iter()
            .map(|x| {
                let mean = x
                    .mean_axis(Axis(0))
                    .with_context(|| "Failed to compute mean")
                    .unwrap();
                // convert to f64
                let mean = mean.mapv(|x| x.into());
                mean.to_vec()
            })
            .collect::<Vec<_>>();

        // reshape vec to 2D array
        let sample_data = Array::from_shape_vec((sample_vec.len(), columns), sample_vec.concat())
            .with_context(|| "Failed to create 2D array")?;

        Ok(sample_data)
    }

    pub fn set_control_drift_value(
        &self,
        array: ArrayView1<f64>,
        num_features: usize,
        drift_profile: &DriftProfile,
        features: &[String],
    ) -> Result<Vec<f64>, anyhow::Error> {
        let mut drift: Vec<f64> = vec![0.0; num_features];
        for (i, feature) in features.iter().enumerate() {
            // check if feature exists
            if !drift_profile.features.contains_key(feature) {
                continue;
            }

            let feature_profile = drift_profile
                .features
                .get(feature)
                .with_context(|| format!("Failed to get feature profile for {}", feature))?;

            let value = array[i];

            if value > feature_profile.three_ucl {
                // insert into zero array
                drift[i] = 4.0;
            } else if value < feature_profile.three_lcl {
                drift[i] = -4.0;
            } else if value < feature_profile.three_ucl && value >= feature_profile.two_ucl {
                drift[i] = 3.0;
            } else if value < feature_profile.two_ucl && value >= feature_profile.one_ucl {
                drift[i] = 2.0;
            } else if value < feature_profile.one_ucl && value > feature_profile.center {
                drift[i] = 1.0;
            } else if value > feature_profile.three_lcl && value <= feature_profile.two_lcl {
                drift[i] = -3.0;
            } else if value > feature_profile.two_lcl && value <= feature_profile.one_lcl {
                drift[i] = -2.0;
            } else if value > feature_profile.one_lcl && value < feature_profile.center {
                drift[i] = -1.0;
            }
        }

        Ok(drift)
    }

    pub fn set_percentage_drift_value(
        &self,
        array: ArrayView1<f64>,
        num_features: usize,
        drift_profile: &DriftProfile,
        features: &[String],
        rule: f64,
    ) -> Result<Vec<f64>, anyhow::Error> {
        let mut drift: Vec<f64> = vec![0.0; num_features];

        for (i, feature) in features.iter().enumerate() {
            // check if feature exists
            if !drift_profile.features.contains_key(feature) {
                continue;
            }
            let feature_profile = drift_profile
                .features
                .get(feature)
                .with_context(|| format!("Failed to get feature profile for {}", feature))?;

            let value = array[i];

            // check if value is within percentage
            let percent_error = ((value - feature_profile.center) / feature_profile.center).abs();

            if percent_error > rule {
                drift[i] = 1.0;
            } else {
                drift[i] = 0.0;
            }
        }

        Ok(drift)
    }

    // Computes drift on a  2D array of data. Typically of n size >= sample_size
    //
    // # Arguments
    //
    // * `array` - A 2D array of f64 values
    // * `features` - A vector of feature names that is mapped to the array (order of features in the order in the array)
    // * `drift_profile` - A monitor profile
    //
    pub fn compute_drift<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>, // n x m data array (features and predictions)
        drift_profile: &DriftProfile,
    ) -> Result<DriftMap, anyhow::Error>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let num_features = drift_profile.features.len();

        // iterate through each feature
        let sample_data = self
            ._sample_data(array, drift_profile.config.sample_size, num_features)
            .with_context(|| "Failed to create sample data")?;

        // iterate through each row of samples
        let drift_array = sample_data
            .axis_iter(Axis(0))
            .into_par_iter()
            .map(|x| {
                // match AlertRules enum

                let drift = if drift_profile
                    .config
                    .alert_config
                    .alert_rule
                    .process
                    .is_some()
                {
                    self.set_control_drift_value(x, num_features, drift_profile, features)
                        .with_context(|| "Failed to set control drift value")
                        .unwrap()
                } else {
                    let rule = drift_profile
                        .config
                        .alert_config
                        .alert_rule
                        .percentage
                        .as_ref()
                        .unwrap()
                        .rule;

                    self.set_percentage_drift_value(x, num_features, drift_profile, features, rule)
                        .with_context(|| "Failed to set percentage drift value")
                        .unwrap()
                };

                drift
            })
            .collect::<Vec<_>>();

        // convert drift array to 2D array
        let drift_array =
            Array::from_shape_vec((drift_array.len(), num_features), drift_array.concat())
                .with_context(|| "Failed to create 2D array")?;

        let mut drift_map = DriftMap::new(
            drift_profile.config.name.clone(),
            drift_profile.config.repository.clone(),
            drift_profile.config.version.clone(),
        );

        for (i, feature) in features.iter().enumerate() {
            let drift = drift_array.column(i);
            let sample = sample_data.column(i);

            let feature_drift = FeatureDrift {
                samples: sample.to_vec(),
                drift: drift.to_vec(),
            };

            drift_map.add_feature(feature.to_string(), feature_drift);
        }

        Ok(drift_map)
    }

    // Samples data for drift detection and returns a vector of DriftServerRecord to send to scouter server
    //
    // # Arguments
    //
    // * `array` - A 2D array of f64 values
    // * `features` - A vector of feature names that is mapped to the array (order of features in the order in the array)
    // * `drift_profile` - A monitor profile
    //
    pub fn sample_data<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>, // n x m data array (features and predictions)
        drift_profile: &DriftProfile,
    ) -> Result<Vec<DriftServerRecord>, anyhow::Error>
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let num_features = drift_profile.features.len();

        // iterate through each feature
        let sample_data = self
            ._sample_data(array, drift_profile.config.sample_size, num_features)
            .with_context(|| "Failed to create sample data")?;

        let mut records = Vec::new();

        for (i, feature) in features.iter().enumerate() {
            let sample = sample_data.column(i);

            sample.iter().for_each(|value| {
                let record = DriftServerRecord {
                    created_at: chrono::Utc::now().naive_utc(),
                    feature: feature.to_string(),
                    value: *value,
                    name: drift_profile.config.name.clone(),
                    repository: drift_profile.config.repository.clone(),
                    version: drift_profile.config.version.clone(),
                };

                records.push(record);
            });
        }

        Ok(records)
    }

    pub fn calculate_drift_from_sample(
        &self,
        features: &[String],
        sample_array: &ArrayView2<f64>, // n x m data array (features and predictions)
        drift_profile: &DriftProfile,
    ) -> Result<Array2<f64>, anyhow::Error> {
        // iterate through each row of samples
        let num_features = drift_profile.features.len();
        let drift_array = sample_array
            .axis_iter(Axis(0))
            .into_par_iter()
            .map(|x| {
                // match AlertRules enum

                let drift = if drift_profile
                    .config
                    .alert_config
                    .alert_rule
                    .process
                    .is_some()
                {
                    self.set_control_drift_value(x, num_features, drift_profile, features)
                        .with_context(|| "Failed to set control drift value")
                        .unwrap()
                } else {
                    let rule = drift_profile
                        .config
                        .alert_config
                        .alert_rule
                        .percentage
                        .as_ref()
                        .unwrap()
                        .rule;

                    self.set_percentage_drift_value(x, num_features, drift_profile, features, rule)
                        .with_context(|| "Failed to set percentage drift value")
                        .unwrap()
                };

                drift
            })
            .collect::<Vec<_>>();

        // convert drift array to 2D array
        let drift_array =
            Array::from_shape_vec((drift_array.len(), num_features), drift_array.concat())
                .with_context(|| "Failed to create 2D array")?;

        Ok(drift_array)
    }

    // creates a feature map from a 2D array
    //
    // # Arguments
    //
    // * `features` - A vector of feature names
    // * `array` - A 2D array of string values
    //
    // # Returns
    //
    // A feature map
    pub fn create_feature_map(
        &self,
        features: &[String],
        array: &[Vec<String>],
    ) -> Result<FeatureMap, anyhow::Error> {
        // check if features and array are the same length
        let shape_match = features.len() == array.len();

        if !shape_match {
            return Err(anyhow::anyhow!("Shape mismatch between features and array"));
        }

        let feature_map = array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let unique = col
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut map = BTreeMap::new();
                for (j, item) in unique.iter().enumerate() {
                    map.insert(item.to_string(), j);

                    // check if j is last index
                    if j == unique.len() - 1 {
                        // insert missing value
                        map.insert("missing".to_string(), j + 1);
                    }
                }

                (features[i].to_string(), map)
            })
            .collect::<BTreeMap<_, _>>();

        Ok(FeatureMap {
            features: feature_map,
        })
    }

    pub fn convert_strings_to_ndarray_f32(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f32>, anyhow::Error>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(anyhow::anyhow!(
                "Features provided do not exist in feature map"
            ));
        }

        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f32)
                    .collect::<Vec<_>>();

                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .with_context(|| "Failed to create 2D array")?
            .t()
            .to_owned();

        Ok(data)
    }

    pub fn convert_strings_to_ndarray_f64(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f64>, anyhow::Error>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(anyhow::anyhow!(
                "Features provided do not exist in feature map"
            ));
        }
        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f64)
                    .collect::<Vec<_>>();
                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .with_context(|| "Failed to create 2D array")?
            .t()
            .to_owned();

        Ok(data)
    }
}

// convert drift array to 2D array

impl Default for Monitor {
    fn default() -> Self {
        Monitor::new()
    }
}

#[cfg(test)]
mod tests {

    use crate::utils::types::{AlertRule, PercentageAlertRule};

    use super::*;
    use approx::relative_eq;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    #[test]
    fn test_create_2d_drift_profile_f32() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        // cast array to f32
        let array = array.mapv(|x| x as f32);

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = Monitor::new();
        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        // test extra funcs that are used in python
        profile.__str__();
        let model_string = profile.model_dump_json();

        let loaded_profile = DriftProfile::load_from_json(model_string);
        assert_eq!(loaded_profile.features.len(), 3);
    }

    #[test]
    fn test_create_2d_drift_profile_f64() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let monitor = Monitor::new();
        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);
    }

    #[test]
    fn test_drift_detect_process() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let monitor = Monitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        // change first 100 rows to 100 at index 1
        let mut array = array.to_owned();
        array.slice_mut(s![0..200, 1]).fill(100.0);

        let drift_map = monitor
            .compute_drift(&features, &array.view(), &profile)
            .unwrap();

        // assert relative
        let feature_1 = drift_map.features.get("feature_2").unwrap();
        assert!(relative_eq!(feature_1.samples[0], 100.0, epsilon = 2.0));

        // convert profile to json and load it back
        let _ = drift_map.model_dump_json();

        // create server records
    }

    #[test]
    fn test_sample_data() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let monitor = Monitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        let server_records = monitor
            .sample_data(&features, &array.view(), &profile)
            .unwrap();

        assert_eq!(server_records.len(), 126);

        // create server records
    }

    #[test]
    fn test_calculate_drift_from_sample() {
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let monitor = Monitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        // change first 100 rows to 100 at index 1
        let mut array = array.to_owned();
        array.slice_mut(s![0..200, 1]).fill(100.0);

        let drift_array = monitor
            .calculate_drift_from_sample(&features, &array.view(), &profile)
            .unwrap();

        // assert relative
        let feature_1 = drift_array.column(1);
        assert!(relative_eq!(feature_1[0], 4.0, epsilon = 2.0));
    }

    #[test]
    fn test_drift_detect_percentage() {
        // create 2d array
        let array = Array::random((1030, 3), Uniform::new(0., 10.));

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let config = DriftConfig::new(
            "name".to_string(),
            "repo".to_string(),
            None,
            None,
            None,
            None,
            Some(AlertRule {
                process: None,
                percentage: Some(PercentageAlertRule { rule: 0.1 }),
            }),
            None,
            None,
            None,
            None,
            None,
        );

        let monitor = Monitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config)
            .unwrap();
        assert_eq!(profile.features.len(), 3);

        // change first 100 rows to 100 at index 1
        let mut array = array.to_owned();
        array.slice_mut(s![0..200, 1]).fill(100.0);

        let drift_map = monitor
            .compute_drift(&features, &array.view(), &profile)
            .unwrap();

        // assert relative
        let feature_1 = drift_map.features.get("feature_2").unwrap();
        assert!(relative_eq!(feature_1.samples[0], 100.0, epsilon = 2.0));

        // convert profile to json and load it back
        let _ = drift_map.model_dump_json();
        let (drift_array, _sample_array, features) = drift_map.to_array().unwrap();

        // check if indices are the same
        for (idx, feature) in features.iter().enumerate() {
            let left = drift_map.features.get(feature).unwrap().drift[0..20].to_vec();

            let right = drift_array.slice(s![0..20, idx]).to_vec();

            assert_eq!(left, right);
        }
    }

    #[test]
    fn test_create_feature_map() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let monitor = Monitor::new();

        let feature_map = monitor
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);
        assert_eq!(feature_map.features.get("feature_2").unwrap().len(), 6);
    }

    #[test]
    fn test_create_array_from_string() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "a".to_string(),
                "a".to_string(),
                "a".to_string(),
                "b".to_string(),
                "b".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let monitor = Monitor::new();

        let feature_map = monitor
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);

        let f32_array = monitor
            .convert_strings_to_ndarray_f32(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f32_array.shape(), &[5, 2]);

        let f64_array = monitor
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f64_array.shape(), &[5, 2]);
    }
}
