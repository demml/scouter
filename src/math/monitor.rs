use crate::types::_types::{FeatureMonitorProfile, MonitorProfile};
use anyhow::{Context, Result};
use chrono::Utc;
use ndarray::prelude::*;
use ndarray::Axis;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt::Debug;

pub struct Monitor {}

impl Monitor {
    pub fn new() -> Self {
        Monitor {}
    }

    //

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

    fn compute_control_limits<F>(
        &self,
        sample_size: usize,
        sample_data: &ArrayView2<F>,
        num_features: usize,
        features: &[String],
    ) -> Result<MonitorProfile, anyhow::Error>
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
        let denominator: F = F::from((sample_size as f64).sqrt()).unwrap();
        let mult_fact = F::from(c4).unwrap() * denominator;

        let base = &stdev / mult_fact;
        let right = &base * F::from(3.0).unwrap();

        let lcl = &means - &right;
        let ucl = &means + &right;
        let center = &means;

        // create monitor profile
        let mut feat_profile = HashMap::new();

        for (i, feature) in features.iter().enumerate() {
            feat_profile.insert(
                feature.to_string(),
                FeatureMonitorProfile {
                    id: feature.to_string(),
                    center: center[i].into(),
                    ucl: ucl[i].into(),
                    lcl: lcl[i].into(),
                    timestamp: Utc::now().to_string(),
                },
            );
        }

        Ok(MonitorProfile {
            features: feat_profile,
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
    pub fn create_2d_monitor_profile<F>(
        &self,
        features: &[String],
        array: ArrayView2<F>,
    ) -> Result<MonitorProfile, anyhow::Error>
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

        // iterate through each feature
        let sample_vec = array
            .axis_chunks_iter(Axis(0), sample_size)
            .into_par_iter()
            .map(|x| {
                let mean = x.mean_axis(Axis(0)).unwrap();
                let stddev = x.std_axis(Axis(0), F::from(1.0).unwrap());

                // append stddev to mean
                let combined = ndarray::concatenate![Axis(0), mean, stddev];
                //mean.remove_axis(Axis(1));

                combined.to_vec()
            })
            .collect::<Vec<_>>();

        // reshape vec to 2D array
        let sample_data =
            Array::from_shape_vec((sample_vec.len(), features.len() * 2), sample_vec.concat())
                .with_context(|| "Failed to create 2D array")?;

        let monitor_profile = self
            .compute_control_limits(sample_size, &sample_data.view(), num_features, features)
            .with_context(|| "Failed to compute control limits")?;

        Ok(monitor_profile)
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor::new()
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    #[test]
    fn test_create_2d_monitor_profile() {
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

        let profile = monitor
            .create_2d_monitor_profile(&features, array.view())
            .unwrap();
        assert_eq!(profile.features.len(), 3);
    }
}