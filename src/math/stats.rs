use crate::types::_types::{
    Distinct, FeatureMonitorProfile, Infinity, Missing, MonitorProfile, Quantiles, Stats,
};
use anyhow::{Context, Result};
use chrono::Utc;
use ndarray::prelude::*;
use ndarray::Axis;
use ndarray_stats::MaybeNan;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::types::n64;
use num_traits::{Float, FromPrimitive, Num};
use numpy::ndarray::ArrayView1;
use rayon::prelude::*;
use std::cmp::Ord;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

/// Compute quantiles for a 1D array.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 2D array of noisy floats.
pub fn compute_quantiles<F>(array: &ArrayView1<F>) -> Result<Quantiles<F>>
where
    F: Num + ndarray_stats::MaybeNan + std::marker::Send + Sync + Clone + Copy,
    <F as ndarray_stats::MaybeNan>::NotNan: Clone,
    <F as ndarray_stats::MaybeNan>::NotNan: Ord,
{
    let axis = Axis(0);
    let qs = &[n64(0.25), n64(0.5), n64(0.75), n64(0.99)];

    let quantiles = qs
        .par_iter()
        .map(|q| {
            array
                .to_owned()
                .quantile_axis_skipnan_mut(axis, *q, &Nearest)
                .unwrap()
                .into_scalar()
        })
        .collect::<Vec<_>>();

    Ok(Quantiles {
        quant_25: quantiles[0],
        quant_50: quantiles[1],
        quant_75: quantiles[2],
        quant_99: quantiles[3],
    })
}

/// Compute the mean for a 1D array.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_mean<F>(array: &ArrayView1<F>) -> Result<F, anyhow::Error>
where
    F: FromPrimitive + Num + Clone,
{
    let mean: F = array.mean().with_context(|| "Failed to compute mean")?;

    Ok(mean)
}

/// Compute the stddev for a 2D array.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_stddev<F>(array: &ArrayView1<F>) -> Result<F>
where
    F: FromPrimitive + Num + Float,
{
    let ddof = F::from(1.0).unwrap();
    Ok(array.std(ddof))
}

/// Compute the min for a 1D array.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_min<F>(array: &ArrayView1<F>) -> Result<F, anyhow::Error>
where
    F: MaybeNan + Num + Clone,
    <F as MaybeNan>::NotNan: Ord,
    F: Into<f64>,
{
    let min = array.min_skipnan().to_owned();
    Ok(min)
}

/// Compute the max for a 1D array.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_max<F>(array: &ArrayView1<F>) -> Result<F, anyhow::Error>
where
    F: MaybeNan + Num + Clone,
    <F as MaybeNan>::NotNan: Ord,
{
    Ok(array.max_skipnan().to_owned())
}

/// Compute the distinct numbers in a 1D matrix.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_distinct<F>(array: &ArrayView1<F>) -> Result<Distinct>
where
    F: std::fmt::Display + Num,
{
    let unique: HashSet<String> = array.iter().map(|x| x.to_string()).collect();
    let count = unique.len() as f64;

    Ok(Distinct {
        count: count as usize,
        percent: count / array.len() as f64,
    })
}

/// Compute the number of missing values in a 1d array of data
///
/// # Arguments
///
/// * `feature_array` - A 1d array of data
///
/// # Returns
/// A 1D array of f64 values.
pub fn count_missing_perc<F>(array: &ArrayView1<F>) -> Result<Missing>
where
    F: Sync + Num + MaybeNan,
{
    let count = array
        .into_par_iter()
        .map(|x| if x.is_nan() { 1.0 } else { 0.0 })
        .to_owned()
        .sum::<f64>();

    let total_count = array.len_of(Axis(0)) as f64;

    Ok(Missing {
        count: count as usize,
        percent: count / total_count,
    })
}

/// Compute the number of infinite values in a 1d array of data
///
/// # Arguments
///
/// * `feature_array` - A 1d array of data
///
/// # Returns
/// * `Result<(f64, f64), String>` - A tuple containing the number of infinite values and the percentage of infinite values
pub fn count_infinity_perc<F>(array: &ArrayView1<F>) -> Result<Infinity>
where
    F: Sync + Num + Float,
{
    let count = array
        .into_par_iter()
        .map(|x| if x.is_infinite() { 1.0 } else { 0.0 })
        .to_owned()
        .sum::<f64>();

    let total_count = array.len_of(Axis(0)) as f64;

    Ok(Infinity {
        count: count as usize,
        percent: count / total_count,
    })
}

/// Compute the base stats for a 1D array of data
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values
///  
/// # Returns
///
/// A tuple containing the mean, standard deviation, min, max, distinct, and quantiles
pub fn compute_base_stats<F>(
    array: &ArrayView1<F>,
) -> Result<(f64, f64, f64, f64, Distinct, Quantiles<F>)>
where
    F: Float + MaybeNan + FromPrimitive + std::fmt::Display + Sync + Send + Num + Clone,
    F: Into<f64>,
    <F as MaybeNan>::NotNan: Ord,
    f64: From<F>,
    <F as MaybeNan>::NotNan: Clone,
{
    let mean = compute_mean(array).with_context(|| "Failed to compute mean")?;
    let stddev = compute_stddev(array).with_context(|| "Failed to compute stddev")?;
    let min = compute_min(array).with_context(|| "Failed to compute min")?;
    let max = compute_max(array).with_context(|| "Failed to compute max")?;
    let distinct = compute_distinct(array).with_context(|| "Failed to compute distinct")?;
    let quantiles = compute_quantiles(array).with_context(|| "Failed to compute quantiles")?;

    Ok((
        mean.into(),
        stddev.into(),
        min.into(),
        max.into(),
        distinct,
        quantiles,
    ))
}

/// Compute the stats for a 1D array of data
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values
///
/// # Returns
///
/// A struct containing the mean, standard deviation, min, max, distinct, infinity, missing, and quantiles
pub fn compute_array_stats<F>(array: &ArrayView1<F>) -> Result<Stats<F>, anyhow::Error>
where
    F: Num + Sync + MaybeNan + FromPrimitive + std::fmt::Display + Send + Float,
    <F as MaybeNan>::NotNan: Ord,
    f64: From<F>,
    <F as MaybeNan>::NotNan: Clone,
{
    let missing = count_missing_perc(array).with_context(|| "Failed to compute missing")?;
    let infinity = count_infinity_perc(array).with_context(|| "Failed to compute infinity")?;

    // check if array has missing or infinite values remove them
    if missing.count > 0 || infinity.count > 0 {
        // remove missing and infinite values from array1
        let array = array
            .into_par_iter()
            .map(|x| x.to_owned())
            .filter(|x| !x.is_nan() && !x.is_infinite())
            .collect::<Vec<_>>();

        // convert vec to array
        let array = Array::from(array);
        let (mean, stddev, min, max, distinct, quantiles) =
            compute_base_stats(&array.view()).with_context(|| "Failed to compute base stats")?;

        Ok(Stats {
            mean,
            standard_dev: stddev,
            min,
            max,
            distinct,
            infinity,
            quantiles,
            missing,
        })
    } else {
        let (mean, stddev, min, max, distinct, quantiles) =
            compute_base_stats(array).with_context(|| "Failed to compute base stats")?;

        Ok(Stats {
            mean,
            standard_dev: stddev,
            min,
            max,
            distinct,
            infinity,
            quantiles,
            missing,
        })
    }
}

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

    use approx::relative_eq;

    #[test]
    fn test_quantiles() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let quantiles = compute_quantiles(&array.column(0).view()).unwrap();

        assert!(relative_eq!(quantiles.quant_25, 2.5, max_relative = 1.0));
        assert!(relative_eq!(quantiles.quant_50, 5.0, max_relative = 1.0));
        assert!(relative_eq!(quantiles.quant_75, 7.5, max_relative = 1.0));
        assert!(relative_eq!(quantiles.quant_99, 9.9, max_relative = 1.0));

        // cast to f32
        let f32_array = array.mapv(|x| x as f32);
        let _quantiles = compute_quantiles(&f32_array.column(0).view()).unwrap();
    }

    #[test]
    fn test_basic_stats() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let mean = compute_mean(&array.column(0).view()).unwrap();
        let stddev = compute_stddev(&array.column(0).view()).unwrap();
        let min = compute_min(&array.column(0).view()).unwrap();
        let max = compute_max(&array.column(0).view()).unwrap();

        assert!(relative_eq!(mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stddev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(max, 9.9, max_relative = 1.0));

        // cast to f32
        let f32_array = array.mapv(|x| x as f32);
        let mean = compute_mean(&f32_array.column(0).view()).unwrap();
        let stddev = compute_stddev(&f32_array.column(0).view()).unwrap();
        let min = compute_min(&f32_array.column(0).view()).unwrap();
        let max = compute_max(&f32_array.column(0).view()).unwrap();

        assert!(relative_eq!(mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stddev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(max, 9.9, max_relative = 1.0));
    }

    #[test]
    fn test_distinct() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let distinct = compute_distinct(&array.column(0).view()).unwrap();

        assert_eq!(distinct.count, 100);
        assert_eq!(distinct.percent, 1.0);
    }

    #[test]
    fn test_missing() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let missing = count_missing_perc(&array.column(0).view()).unwrap();

        assert_eq!(missing.count, 0);
        assert_eq!(missing.percent, 0.0);
    }

    #[test]
    fn test_infinity() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let infinity = count_infinity_perc(&array.column(0).view()).unwrap();

        assert_eq!(infinity.count, 0);
        assert_eq!(infinity.percent, 0.0);
    }

    #[test]
    fn test_array_stats() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let stats = compute_array_stats(&array.column(0).view()).unwrap();

        assert!(relative_eq!(stats.mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stats.standard_dev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(stats.min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(stats.max, 9.9, max_relative = 1.0));
        assert_eq!(stats.distinct.count, 100);
        assert_eq!(stats.distinct.percent, 1.0);
        assert_eq!(stats.missing.count, 0);
        assert_eq!(stats.missing.percent, 0.0);
        assert_eq!(stats.infinity.count, 0);
        assert_eq!(stats.infinity.percent, 0.0);
    }

    #[test]
    fn test_array_stats_with_missing() {
        // create 2d array
        let mut array = Array::random((100, 1), Uniform::new(0., 10.));

        // add missing values
        array[[0, 0]] = std::f64::NAN;
        array[[1, 0]] = std::f64::NAN;

        let stats = compute_array_stats(&array.column(0).view()).unwrap();

        assert!(relative_eq!(stats.mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stats.standard_dev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(stats.min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(stats.max, 9.9, max_relative = 1.0));
        assert_eq!(stats.distinct.count, 98);
        assert_eq!(stats.distinct.percent, 1.0);
        assert_eq!(stats.missing.count, 2);
        assert_eq!(stats.missing.percent, 0.02);
        assert_eq!(stats.infinity.count, 0);
        assert_eq!(stats.infinity.percent, 0.0);
    }

    #[test]
    fn test_array_stats_with_infinity() {
        // create 2d array
        let mut array = Array::random((100, 1), Uniform::new(0., 10.));

        // add infinite values
        array[[0, 0]] = std::f64::INFINITY;
        array[[1, 0]] = std::f64::NEG_INFINITY;

        let stats = compute_array_stats(&array.column(0).view()).unwrap();

        assert!(relative_eq!(stats.mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stats.standard_dev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(stats.min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(stats.max, 9.9, max_relative = 1.0));
        assert_eq!(stats.distinct.count, 98);
        assert_eq!(stats.distinct.percent, 1.0);
        assert_eq!(stats.missing.count, 0);
        assert_eq!(stats.missing.percent, 0.0);
        assert_eq!(stats.infinity.count, 2);
        assert_eq!(stats.infinity.percent, 0.02);
    }

    #[test]
    fn test_array_stats_with_missing_and_infinity() {
        // create 2d array
        let mut array = Array::random((100, 1), Uniform::new(0., 10.));

        // add missing and infinite values
        array[[0, 0]] = std::f64::NAN;
        array[[1, 0]] = std::f64::INFINITY;

        let stats = compute_array_stats(&array.column(0).view()).unwrap();

        assert!(relative_eq!(stats.mean, 5.0, max_relative = 1.0));
        assert!(relative_eq!(stats.standard_dev, 2.89, max_relative = 1.0));
        assert!(relative_eq!(stats.min, 0.0, max_relative = 1.0));
        assert!(relative_eq!(stats.max, 9.9, max_relative = 1.0));
        assert_eq!(stats.distinct.count, 98);
        assert_eq!(stats.distinct.percent, 1.0);
        assert_eq!(stats.missing.count, 1);
        assert_eq!(stats.missing.percent, 0.01);
        assert_eq!(stats.infinity.count, 1);
        assert_eq!(stats.infinity.percent, 0.01);
    }

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
