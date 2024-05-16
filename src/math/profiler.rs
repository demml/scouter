use crate::types::_types::{DataProfile, Distinct, FeatureDataProfile, Histogram, Quantiles};
use anyhow::{Context, Result};
use chrono::Utc;
use ndarray::prelude::*;
use ndarray::Axis;
use ndarray_stats::MaybeNan;

use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::types::n64;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::cmp::Ord;
use std::collections::HashMap;
use std::collections::HashSet;

pub struct Profiler {
    bin_size: usize,
}

impl Profiler {
    pub fn new(bin_size: usize) -> Self {
        Profiler { bin_size }
    }

    /// Compute quantiles for a 2D array.
    ///
    /// # Arguments
    ///
    /// * `array` - A 1D array of f64 values.
    ///
    /// # Returns
    ///
    /// A 2D array of noisy floats.
    pub fn compute_quantiles<F>(&self, array: &ArrayView2<F>) -> Result<Vec<Vec<F>>>
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
                    .to_vec()
            })
            .collect::<Vec<_>>();

        Ok(quantiles)
    }

    /// Compute the mean for a 2D array.
    ///
    /// # Arguments
    ///
    /// * `array` - A 2D array of values.
    ///
    /// # Returns
    ///
    /// A 1D array of f64 values.
    pub fn compute_mean<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, anyhow::Error>
    where
        F: FromPrimitive + Num + Clone,
    {
        let mean = array
            .mean_axis(Axis(0))
            .with_context(|| "Failed to compute mean")?;

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
    pub fn compute_stddev<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>>
    where
        F: FromPrimitive + Num + Float,
    {
        let ddof = F::from(1.0).unwrap();
        let stddev = array.std_axis(Axis(0), ddof);
        Ok(stddev)
    }

    /// Compute the min for a 2D array.
    ///
    /// # Arguments
    ///
    /// * `array` - A 2D array of values.
    ///
    /// # Returns
    ///
    /// A 1D array of values.
    pub fn compute_min<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, anyhow::Error>
    where
        F: MaybeNan + Num + Clone,
        <F as MaybeNan>::NotNan: Ord,
        F: Into<f64>,
    {
        let min = array.map_axis(Axis(0), |a| a.min_skipnan().to_owned());
        Ok(min)
    }

    /// Compute the max for a 2D array.
    ///
    /// # Arguments
    ///
    /// * `array` - A 2D array of values.
    ///
    /// # Returns
    ///
    /// A 1D array of values.
    pub fn compute_max<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, anyhow::Error>
    where
        F: MaybeNan + Num + Clone,
        <F as MaybeNan>::NotNan: Ord,
        F: Into<f64>,
    {
        let max = array.map_axis(Axis(0), |a| a.max_skipnan().to_owned());
        Ok(max)
    }

    /// Compute the distinct numbers in a 2D matrix.
    ///
    /// # Arguments
    ///
    /// * `array` - A 2D array of values.
    ///
    /// # Returns
    ///
    /// A 1D array of values.
    pub fn compute_distinct<F>(&self, array: &ArrayView2<F>) -> Result<Vec<Distinct>>
    where
        F: std::fmt::Display + Num,
    {
        let unique: Vec<Distinct> = array
            .axis_iter(Axis(1))
            .map(|x| {
                let hash = x.iter().map(|x| x.to_string()).collect::<HashSet<String>>();
                Distinct {
                    count: hash.len(),
                    percent: hash.len() as f64 / x.len() as f64,
                }
            })
            .collect();

        Ok(unique)
    }

    /// Compute the histogram and bins from a 2D matrix.
    ///
    /// # Arguments
    ///
    /// * `array` - A 2D array of values.
    ///
    /// # Returns
    ///
    pub fn compute_bins<F>(&self, array: &ArrayView1<F>) -> Result<Vec<f64>, anyhow::Error>
    where
        F: Float + Num + core::ops::Sub,
        f64: From<F>,
    {
        // find the min and max of the data

        let max: f64 = array
            .max()
            .with_context(|| "Failed to compute max")?
            .to_owned()
            .into();
        let min: f64 = array
            .min()
            .with_context(|| "Failed to compute min")?
            .to_owned()
            .into();

        // create a vector of bins
        let mut bins = Vec::<f64>::with_capacity(self.bin_size);

        // compute the bin width
        let bin_width = (max - min) / self.bin_size as f64;

        // create the bins
        for i in 0..self.bin_size {
            bins.push(min + bin_width * i as f64);
        }

        // return the bins
        Ok(bins)
    }

    pub fn compute_bin_counts<F>(
        &self,
        array: &ArrayView1<F>,
        bins: &[f64],
    ) -> Result<Vec<i32>, anyhow::Error>
    where
        F: Num + ndarray_stats::MaybeNan + std::marker::Send + Sync + Clone + Copy,
        f64: From<F>,
    {
        // create a vector of size bins
        let mut bin_counts = vec![0; bins.len()];
        let max_bin = bins.last().unwrap();

        array.map(|datum| {
            // iterate over the bins
            for (i, bin) in bins.iter().enumerate() {
                let val: f64 = datum.to_owned().into();

                if bin != max_bin {
                    // check if datum is between bin and next bin
                    if &val >= bin && val < bins[i + 1] {
                        bin_counts[i] += 1;
                        break;
                    }
                    continue;
                } else if bin == max_bin {
                    if &val > bin {
                        bin_counts[i] += 1;
                        break;
                    }
                    continue;
                } else {
                    continue;
                }
            }
        });

        Ok(bin_counts)
    }

    pub fn compute_histogram<F>(
        &self,
        array: &ArrayView2<F>,
        features: &[String],
    ) -> Result<HashMap<String, Histogram>, anyhow::Error>
    where
        F: Num
            + ndarray_stats::MaybeNan
            + std::marker::Send
            + Sync
            + Clone
            + Copy
            + num_traits::Float,
        f64: From<F>,
    {
        let hist: HashMap<String, Histogram> = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .enumerate()
            .map(|(idx, x)| {
                let bins = self
                    .compute_bins(&x)
                    .with_context(|| "Failed to compute bins")
                    .unwrap();
                let bin_counts = self
                    .compute_bin_counts(&x, &bins)
                    .with_context(|| "Failed to compute bin counts")
                    .unwrap();
                (features[idx].clone(), Histogram { bins, bin_counts })
                // return
            })
            .collect();

        Ok(hist)
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
    pub fn compute_stats<F>(
        &self,
        features: &[String],
        array: &ArrayView2<F>,
    ) -> Result<DataProfile, anyhow::Error>
    where
        F: Float
            + MaybeNan
            + FromPrimitive
            + std::fmt::Display
            + Sync
            + Send
            + Num
            + Clone
            + std::fmt::Debug,
        F: Into<f64>,
        <F as MaybeNan>::NotNan: Ord,
        f64: From<F>,
        <F as MaybeNan>::NotNan: Clone,
    {
        let means = self
            .compute_mean(array)
            .with_context(|| "Failed to compute mean")?;
        let stddevs = self
            .compute_stddev(array)
            .with_context(|| "Failed to compute stddev")?;
        let quantiles = self
            .compute_quantiles(array)
            .with_context(|| "Failed to compute quantiles")?;
        let mins = self
            .compute_min(array)
            .with_context(|| "Failed to compute min")?;
        let maxs = self
            .compute_max(array)
            .with_context(|| "Failed to compute max")?;
        let distinct = self
            .compute_distinct(array)
            .with_context(|| "Failed to compute distinct")?;

        let hist = self
            .compute_histogram(array, features)
            .with_context(|| "Failed to compute histogram")?;

        // loop over list
        let mut profiles = HashMap::new();
        for i in 0..features.len() {
            let mean = &means[i];
            let stddev = &stddevs[i];
            let min = &mins[i];
            let max = &maxs[i];
            let q25 = &quantiles[0][i];
            let q50 = &quantiles[1][i];
            let q75 = &quantiles[2][i];
            let q99 = &quantiles[3][i];
            let dist = &distinct[i];

            let profile = FeatureDataProfile {
                id: features[i].clone(),
                mean: f64::from(*mean),
                stddev: f64::from(*stddev),
                min: f64::from(*min),
                max: f64::from(*max),
                timestamp: Utc::now().to_string(),
                distinct: Distinct {
                    count: dist.count,
                    percent: dist.percent,
                },
                quantiles: Quantiles {
                    q25: f64::from(*q25),
                    q50: f64::from(*q50),
                    q75: f64::from(*q75),
                    q99: f64::from(*q99),
                },
                histogram: hist[&features[i]].clone(),
            };

            profiles.insert(features[i].clone(), profile);
        }

        Ok(DataProfile { features: profiles })
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Profiler::new(20)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use ndarray::Array;
    use ndarray::{concatenate, Axis};
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    use approx::relative_eq;

    #[test]
    fn test_profile_creation_f64() {
        // create 2d array
        let array1 = Array::random((1000, 1), Uniform::new(0., 1.));
        let array2 = Array::random((1000, 1), Uniform::new(1., 2.));
        let array3 = Array::random((1000, 1), Uniform::new(2., 3.));

        let array = concatenate![Axis(1), array1, array2, array3];
        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        let profiler = Profiler::default();

        let profile = profiler.compute_stats(&features, &array.view()).unwrap();

        println!("{:?}", profile);

        assert_eq!(profile.features.len(), 3);
        assert_eq!(profile.features["feature_1"].id, "feature_1");
        assert_eq!(profile.features["feature_2"].id, "feature_2");
        assert_eq!(profile.features["feature_3"].id, "feature_3");

        // check mean
        assert!(relative_eq!(
            profile.features["feature_1"].mean,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_2"].mean,
            1.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_3"].mean,
            2.5,
            epsilon = 0.05
        ));

        // check quantiles
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q25,
            0.25,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q50,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q75,
            0.75,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q99,
            0.99,
            epsilon = 0.05
        ));
    }

    #[test]
    fn test_profile_creation_f32() {
        // create 2d array
        let array1 = Array::random((1000, 1), Uniform::new(0., 1.));
        let array2 = Array::random((1000, 1), Uniform::new(1., 2.));
        let array3 = Array::random((1000, 1), Uniform::new(2., 3.));

        let array = concatenate![Axis(1), array1, array2, array3];
        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        // cast array to f32
        let array = array.mapv(|x| x as f32);

        let profiler = Profiler::default();

        let profile = profiler.compute_stats(&features, &array.view()).unwrap();

        println!("{:?}", profile);

        assert_eq!(profile.features.len(), 3);
        assert_eq!(profile.features["feature_1"].id, "feature_1");
        assert_eq!(profile.features["feature_2"].id, "feature_2");
        assert_eq!(profile.features["feature_3"].id, "feature_3");

        // check mean
        assert!(relative_eq!(
            profile.features["feature_1"].mean,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_2"].mean,
            1.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_3"].mean,
            2.5,
            epsilon = 0.05
        ));

        // check quantiles
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q25,
            0.25,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q50,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q75,
            0.75,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile.features["feature_1"].quantiles.q99,
            0.99,
            epsilon = 0.05
        ));
    }
}
