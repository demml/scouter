use crate::types::_types::{Distinct, Infinity, Missing, MonitorProfile, Quantiles, Stats};
use anyhow::{Context, Result};
use ndarray::prelude::*;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::types::n64;
use numpy::ndarray::ArrayView1;
use rayon::prelude::*;
use std::collections::HashSet;

/// Compute quantiles for a 1D array.
///
/// # Arguments
///
/// * `array` - A 1D array of f64 values.
///
/// # Returns
///
/// A 2D array of noisy floats.
pub fn compute_quantiles(array: &ArrayView1<f64>) -> Result<Quantiles> {
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
pub fn compute_mean(array: &ArrayView1<f64>) -> Result<f64, anyhow::Error> {
    array.mean().with_context(|| "Failed to compute mean")
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
pub fn compute_stddev(array: &ArrayView1<f64>) -> Result<f64> {
    Ok(array.std(1.0))
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
pub fn compute_min(array: &ArrayView1<f64>) -> Result<f64, anyhow::Error> {
    Ok(array.min_skipnan().to_owned())
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
pub fn compute_max(array: &ArrayView1<f64>) -> Result<f64, anyhow::Error> {
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
pub fn compute_distinct(array: &ArrayView1<f64>) -> Result<Distinct> {
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
pub fn count_missing_perc(array: &ArrayView1<f64>) -> Result<Missing> {
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
pub fn count_infinity_perc(array: &ArrayView1<f64>) -> Result<Infinity> {
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
pub fn compute_base_stats(
    array: &ArrayView1<f64>,
) -> Result<(f64, f64, f64, f64, Distinct, Quantiles)> {
    let mean = compute_mean(array).with_context(|| "Failed to compute mean")?;
    let stddev = compute_stddev(array).with_context(|| "Failed to compute stddev")?;
    let min = compute_min(array).with_context(|| "Failed to compute min")?;
    let max = compute_max(array).with_context(|| "Failed to compute max")?;
    let distinct = compute_distinct(array).with_context(|| "Failed to compute distinct")?;
    let quantiles = compute_quantiles(array).with_context(|| "Failed to compute quantiles")?;

    Ok((mean, stddev, min, max, distinct, quantiles))
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
pub fn compute_array_stats(array: &ArrayView1<f64>) -> Result<Stats, anyhow::Error> {
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
    //c4 is asymptotically equivalent to (4n-3)/(4n-4)
    let n = number as f64;
    let left = 4.0 * n - 4.0;
    let right = 4.0 * n - 3.0;
    let c4 = left / right;

    c4
}

fn compute_control_limits(
    sample_size: usize,
    sample_data: &ArrayView2<f64>,
) -> Result<MonitorProfile, anyhow::Error> {
    let c4 = compute_c4(sample_size) as f64;

    // get mean of 1st col
    let sample_xbar =
        compute_mean(&sample_data.column(0)).with_context(|| "Failed to compute sample xbar")?;

    // get std of 2nd col
    let sample_sigma =
        compute_stddev(&sample_data.column(1)).with_context(|| "Failed to compute sample sigma")?;

    // compute xbar ucl
    let right = 3.0 * (sample_sigma / (c4 * (sample_size as f64).sqrt()));

    Ok(MonitorProfile {
        ucl: sample_xbar + right,
        lcl: sample_xbar - right,
        center: sample_xbar,
    })
}

pub fn create_monitor_profile(
    array: &ArrayView1<f64>,
    sample_size: usize,
) -> Result<MonitorProfile, anyhow::Error> {
    // create a 2d array of chunks (xbar, sigma) and return 2d array of xbar and sigma

    let sample_data = array
        .axis_chunks_iter(Axis(0), sample_size)
        .into_par_iter()
        .filter(|x| x.len() == sample_size)
        .map(|x| {
            let mean = compute_mean(&x).unwrap();
            let stddev = compute_stddev(&x).unwrap();
            vec![mean, stddev]
        })
        .collect::<Vec<_>>()
        .concat();

    // create 2d array of xbar and sigma
    let _sample_data = Array::from_shape_vec((sample_data.len() / 2, 2), sample_data).unwrap();

    let profile = compute_control_limits(sample_size, &_sample_data.view())
        .with_context(|| "Failed to compute control limits")?;

    Ok(profile)
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
    fn test_create_monitor_profile() {
        // create 2d array
        let array = Array::random((100, 1), Uniform::new(0., 10.));

        let profile = create_monitor_profile(&array.column(0).view(), 10).unwrap();
        println!("{:?}", profile);
        assert!(relative_eq!(profile.ucl, 5.0, max_relative = 1.0));
        assert!(relative_eq!(profile.lcl, 5.0, max_relative = 1.0));
        assert!(relative_eq!(profile.center, 5.0, max_relative = 1.0));
    }
}
