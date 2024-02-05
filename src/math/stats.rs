use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::math::logger::Logger;
use crate::math::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
use anyhow::{Context, Result};
use medians::{MStats, Median, Medianf64};
use ndarray::prelude::*;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::prelude::*;
use noisy_float::types::n64;
use num_traits::Float;
use numpy::ndarray::{aview1, ArrayView1, ArrayView2};
use rayon::prelude::*;
use std::collections::HashSet;

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
pub fn compute_mean(array: &ArrayView2<f64>) -> Result<Array1<f64>, anyhow::Error> {
    array
        .mean_axis(Axis(0))
        .with_context(|| "Failed to calculate mean")
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
pub fn compute_stddev(array: &ArrayView2<f64>) -> Result<Array1<f64>, anyhow::Error> {
    array
        .std_axis(Axis(0), 1.0)
        .with_context(|| "Failed to calculate stddev")
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
pub fn compute_min(array: &ArrayView2<f64>) -> Result<Array1<f64>, anyhow::Error> {
    array
        .map_axis(Axis(0), |view| *view.min().unwrap())
        .with_context(|| "Failed to calculate min")
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
pub fn compute_max(array: &ArrayView2<f64>) -> Result<Array1<f64>, anyhow::Error> {
    array
        .map_axis(Axis(0), |view| *view.max().unwrap())
        .with_context(|| "Failed to calculate max")
}

/// Compute the number of distinct values in a 1d array of data
///
/// # Arguments
///
/// * `feature_array` - A 1d array of data
///
/// # Returns
/// * `Result<Distinct, String>` - A tuple containing the number of distinct values and the percentage of distinct values
pub fn count_distinct<T: Send + Sync + std::fmt::Display>(
    feature_array: &ArrayView1<T>,
) -> Result<Distinct, String> {
    let unique: HashSet<String> = feature_array
        .into_par_iter()
        .map(|x| x.to_string())
        .collect();
    let count = unique.len();
    let count_perc = count as f64 / feature_array.len() as f64;

    Ok(Distinct {
        count,
        percent: count_perc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_rand::rand_distr::Normal;
    use ndarray_rand::RandomExt;
    use numpy::ndarray::{arr1, Array2};
    use rstats::Median;

    #[test]
    fn test_count_distinct() {
        // Test floats

        //test ints
        let array = arr1(&[1, 2, 3, 4, 5, 1, 1, 1, 1, 1, 1]);
        let distinct = count_distinct(&array.view()).unwrap();

        assert_eq!(distinct.count, 5);
        assert_eq!(distinct.percent, 5.0 / 11.0);

        // test string
        let array = arr1(&["a", "b", "c", "d", "e", "a", "a", "a", "a", "a", "a"]);
        let distinct = count_distinct(&array.view()).unwrap();

        assert_eq!(distinct.count, 5);
        assert_eq!(distinct.percent, 5.0 / 11.0);

        // test float
        let array = arr1(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 1.2, 2.3, 3.4, 8.9]);
        let distinct = count_distinct(&array.view()).unwrap();

        assert_eq!(distinct.count, 10);
        assert_eq!(distinct.percent, 10.0 / 10.0);
    }

    #[test]
    fn test_count_missing() {
        let test_array = [
            Some(1.0),
            Some(2.0),
            Some(3.0),
            None,
            Some(5.0),
            Some(1.0),
            Some(1.0),
            Some(1.0),
            Some(1.2),
            Some(-1.5),
            Some(100.0),
        ];

        let (count, count_perc) = count_missing(&test_array).unwrap();

        assert_eq!(count, 1);
        assert_eq!(count_perc, 1.0 / 11.0);
    }

    #[test]
    fn test_count_infinity() {
        let test_array = arr1(&[1.0, 2.0, 3.0, 4.0, 5.0, 1.0, f64::INFINITY, f64::INFINITY]);

        let infinity = count_infinity(&test_array.view()).unwrap();

        assert_eq!(infinity.count, 2);
        assert_eq!(infinity.percent, 2.0 / 8.0);
    }

    #[test]
    fn test_median() {
        let v1 = vec![
            1_u8, 2, 2, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6,
        ];

        let median = v1.as_slice().medstats(|&x| x.into()).expect("median");

        println!("median: {}", median);
    }

    #[test]
    fn test_compute_2d_array() {
        let array = ndarray::arr2(&[
            [1.0, 2.0, 3.0, 4.0, 5.0],
            [1.0, 2.0, 3.0, 4.0, 5.0],
            [1.0, f64::INFINITY, 3.0, 4.0, 5.0],
        ]);
        let feature_names = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];

        let features = compute_2d_array_stats(&feature_names, &array.view(), &None, &None).unwrap();
        assert_eq!(features.len(), 5);

        let array = Array2::random((1_000, 10), Normal::new(0.0, 1.0).unwrap());
        let feature_names = (0..10).map(|x| x.to_string()).collect::<Vec<_>>();
        let features = compute_2d_array_stats(&feature_names, &array.view(), &None, &None).unwrap();
        assert_eq!(features.len(), 10);
    }
}
