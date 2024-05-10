use crate::types::_types::{
    Distinct, FeatureMonitorProfile, Infinity, Missing, MonitorProfile, Quantiles, Stats,
};
use anyhow::{Context, Result};
use chrono::Utc;
use ndarray::arr3;
use ndarray::prelude::*;
use ndarray::OwnedRepr;
use ndarray::{arr2, concatenate, Axis};
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use ndarray_stats::{MaybeNan, MaybeNanExt};
use noisy_float::types::n64;
use num_traits::{Float, FromPrimitive, Num};
use numpy::ndarray::ArrayView1;
use rayon::prelude::*;
use serde_json::to_vec;
use std::cmp::Ord;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

// Checks if features are provided, if not, generate feature names
pub fn check_features<F>(
    features: &Vec<String>,
    array: ArrayView2<F>,
) -> Result<Vec<String>, anyhow::Error> {
    if features.is_empty() {
        let mut feature_names = Vec::new();
        for i in 0..array.ncols() {
            feature_names.push(format!("feature_{}", i));
        }

        Ok(feature_names)
    } else {
        Ok(features.to_owned())
    }
}

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

fn compute_c4(number: usize) -> f64 {
    //c4 is asymptotically equivalent to (4n-4)/(4n-3)
    let n = number as f64;
    let left = 4.0 * n - 4.0;
    let right = 4.0 * n - 3.0;
    left / right
}

fn compute_control_limits<F>(
    id: &str,
    sample_size: usize,
    sample_data: &ArrayView2<F>,
    c4: f64,
) -> Result<FeatureMonitorProfile, anyhow::Error>
where
    F: FromPrimitive + Num + Clone + Float + Debug,
    F: Into<f64>,
{
    // get mean of 1st col
    let means = sample_data.mean_axis(Axis(0)).unwrap();

    println!("{:?}", means);

    let sample_xbar: f64 = compute_mean(&sample_data.column(0))
        .with_context(|| "Failed to compute sample xbar")?
        .into();

    // get std of 2nd col
    let sample_sigma: f64 = compute_stddev(&sample_data.column(1))
        .with_context(|| "Failed to compute sample sigma")?
        .into();

    let denominator: f64 = (sample_size as f64).sqrt();
    let x: f64 = sample_sigma / (c4 * denominator);

    // compute xbar ucl
    let right = 3.0 * x;

    Ok(FeatureMonitorProfile {
        id: id.to_string(),
        ucl: sample_xbar + right,
        lcl: sample_xbar - right,
        center: sample_xbar,
        timestamp: Utc::now().to_string(),
    })
}

pub fn create_2d_monitor_profile<F>(
    features: &Vec<String>,
    array: ArrayView2<F>,
) -> Result<MonitorProfile, anyhow::Error>
where
    F: Float + Sync + FromPrimitive + Send + Num + Debug + num_traits::Zero,
    F: Into<f64>,
{
    let arr_features =
        check_features(features, array).with_context(|| "Failed to get feature names")?;

    // get sample size (we can refine this later on)
    let sample_size = if array.len_of(Axis(0)) < 1000 {
        100
    } else {
        25
    };

    let c4 = compute_c4(sample_size);

    // iterate through each feature
    let sample_vec = array
        .axis_chunks_iter(Axis(0), sample_size)
        .into_par_iter()
        .map(|x| {
            let mut mean = x.mean_axis(Axis(0)).unwrap().to_vec();
            let mut stddev = x.std_axis(Axis(0), F::from(1.0).unwrap()).to_vec();

            // concatenate vecs
            mean.append(&mut stddev);

            mean
        })
        .collect::<Vec<_>>();

    let sample_data = Array::from_shape_vec(
        (sample_vec.len(), arr_features.len() * 2),
        sample_vec.concat(),
    )
    .unwrap();

    let sample_mean = sample_data.mean_axis(Axis(0));

    let slice = sample_data
        .slice_axis(
            Axis(1),
            ndarray::Slice {
                start: 3,
                end: Some(6),
                step: 1,
            },
        )
        .mean_axis(Axis(0));

    // iterate through each column and create a monitor profile

    //let sample_vec = array
    //    .axis_chunks_iter(Axis(0), sample_size)
    //    .into_par_iter()
    //    .map(|x| {
    //        let mean = x.mean_axis(Axis(0)).unwrap();
    //        let stddev = x.std_axis(Axis(0), F::from(1.0).unwrap());
    //
    //        vec![mean, stddev]
    //    })
    //    .collect::<Vec<_>>()
    //    .concat();
    //
    //// create 2d array of mean and stddev
    //let sample_data = Array::from_shape_vec((sample_vec.len() / 2, 2), sample_vec).unwrap();

    let a = HashMap::new();
    Ok(MonitorProfile { features: a })
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

        let profile = create_2d_monitor_profile(&features, array.view()).unwrap();
        assert_eq!(profile.features.len(), 3);
    }

    #[test]
    fn test_iter_axis_chunks() {
        // create 2d array
        let array = arr2(&[
            [1., 2., 3.],
            [1., f64::NAN, 6.],
            [1., 8., 9.],
            [1., 11., 12.],
        ]);

        array.axis_chunks_iter(Axis(0), 3).for_each(|x| {
            println!("{:?}", x);

            let mean = x.mean_axis(Axis(0)).unwrap();
            let stddev = x.std_axis(Axis(0), 1.0);

            println!("mean {:?}", mean);
        });
        assert_eq!(2, 3);
    }
}
