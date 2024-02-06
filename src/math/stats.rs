use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::types::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
use anyhow::{Context, Result};
use ndarray::prelude::*;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::prelude::*;
use noisy_float::types::n64;
use num_traits::Float;
use numpy::ndarray::{aview1, ArrayView1, ArrayView2};
use rayon::prelude::*;
use std::collections::HashSet;
use tracing::{debug, error, info, span, warn, Level};

/// Compute quantiles for a 2D array.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 2D array of noisy floats.
pub fn compute_quantiles(array: &ArrayView2<f64>) -> Result<Array2<N64>, anyhow::Error> {
    let axis = Axis(0);
    let qs = &[n64(0.25), n64(0.5), n64(0.75), n64(0.99)];
    array
        .map(|x| n64(*x))
        .quantiles_axis_mut(axis, &aview1(qs), &Nearest)
        .with_context(|| "Failed to compute quantiles")
}

/// Compute the mean for a 2D array.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_mean(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    array
        .mean_axis(Axis(0))
        .with_context(|| "Failed to compute mean")
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
pub fn compute_stddev(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    Ok(array.std_axis(Axis(0), 1.0))
}

/// Compute the min for a 2D array.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_min(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    Ok(array.map_axis(Axis(0), |view| *view.min().unwrap()))
}

/// Compute the max for a 2D array.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_max(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    Ok(array.map_axis(Axis(0), |view| *view.max().unwrap()))
}

/// Compute the distinct numbers in a 2D matrix.
///
/// # Arguments
///
/// * `array` - A 2D array of f64 values.
///
/// # Returns
///
/// A 1D array of f64 values.
pub fn compute_distinct(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    Ok(array.map_axis(Axis(0), |view| {
        let unique: HashSet<String> = view.iter().map(|x| x.to_string()).collect();
        let count = unique.len() as f64;
        count / view.len() as f64
    }))
}

/// Compute the number of missing values in a 1d array of data
///
/// # Arguments
///
/// * `feature_array` - A 1d array of data
///
/// # Returns
/// A 1D array of f64 values.
pub fn count_missing_perc(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    let count = array.map_axis(Axis(0), |view| {
        view.iter().filter(|x| x.is_nan()).count() as f64
    });

    let total_count = array.len_of(Axis(0)) as f64;

    Ok(count / total_count)
}

/// Compute the number of infinite values in a 1d array of data
///
/// # Arguments
///
/// * `feature_array` - A 1d array of data
///
/// # Returns
/// * `Result<(f64, f64), String>` - A tuple containing the number of infinite values and the percentage of infinite values
pub fn count_infinity_perc(array: &ArrayView2<f64>) -> Result<Array1<f64>> {
    let count = array.map_axis(Axis(0), |view| {
        view.iter().filter(|x| x.is_infinite()).count() as f64
    });
    let total_count = array.len_of(Axis(0)) as f64;

    Ok(count / total_count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    use approx::relative_eq;

    #[test]
    fn test_stats() {
        // create 2d array
        let array = Array::random((100, 10), Uniform::new(0., 10.));

        let means = compute_mean(&array.view()).unwrap();
        let stds = compute_stddev(&array.view()).unwrap();
        let mins = compute_min(&array.view()).unwrap();
        let maxs = compute_max(&array.view()).unwrap();
        let quantiles = compute_quantiles(&array.view()).unwrap();
        let distinct = compute_distinct(&array.view()).unwrap();
        let missing = count_missing_perc(&array.view()).unwrap();
        let infinity = count_infinity_perc(&array.view()).unwrap();

        // test means
        assert!(means.len() == 10);

        // iterate over means
        for mean in means.iter() {
            assert!(relative_eq!(
                mean,
                &means.mean().unwrap(),
                max_relative = 1.0
            ));
        }

        // test stds
        assert!(stds.len() == 10);

        // iterate over stds
        for std in stds.iter() {
            assert!(relative_eq!(std, &stds.mean().unwrap(), max_relative = 1.0));
        }

        // test mins
        print!("mins: {:?}", mins);
        assert!(mins.len() == 11);
    }
}
