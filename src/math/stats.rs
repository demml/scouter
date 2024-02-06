use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::types::types::{Bin, Distinct, FeatureStat, Infinity, Missing, Quantiles, Stats};
use anyhow::{Context, Result};
use ndarray::prelude::*;
use ndarray::DataMut;
use ndarray::ViewRepr;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::prelude::*;
use noisy_float::types::n64;
use num_traits::Float;
use numpy::ndarray::{aview1, ArrayView1, ArrayView2};
use rayon::prelude::*;
use std::collections::HashSet;
use tracing::{debug, error, info, span, warn, Level};
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
        .map(|x| {
            if x.is_nan() {
                return 1.0;
            } else {
                return 0.0;
            }
        })
        .to_owned()
        .sum::<f64>();

    let total_count = array.len_of(Axis(0)) as f64;

    Ok(Missing {
        count: count as usize,
        percent: count / total_count as f64,
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
        .map(|x| {
            if x.is_infinite() {
                return 1.0;
            } else {
                return 0.0;
            }
        })
        .to_owned()
        .sum::<f64>();

    let total_count = array.len_of(Axis(0)) as f64;

    Ok(Infinity {
        count: count as usize,
        percent: count / total_count as f64,
    })
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
}
