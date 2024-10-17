use crate::core::error::ProfilerError;
use crate::core::profile::types::{Distinct, FeatureProfile, Histogram, NumericStats, Quantiles};
use ndarray::prelude::*;
use ndarray::Axis;
use ndarray_stats::MaybeNan;

use crate::core::profile::types::DataProfile;
use crate::core::stats::compute_feature_correlations;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::types::n64;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::cmp::Ord;
use std::collections::{BTreeMap, HashMap, HashSet};
pub struct NumProfiler {}

impl NumProfiler {
    pub fn new() -> Self {
        NumProfiler {}
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
    pub fn compute_quantiles<F>(&self, array: &ArrayView2<F>) -> Result<Vec<Vec<F>>, ProfilerError>
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
    pub fn compute_mean<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, ProfilerError>
    where
        F: FromPrimitive + Num + Clone,
    {
        let mean = array.mean_axis(Axis(0)).ok_or(ProfilerError::MeanError)?;

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
    pub fn compute_stddev<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, ProfilerError>
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
    pub fn compute_min<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, ProfilerError>
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
    pub fn compute_max<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, ProfilerError>
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
    pub fn compute_distinct<F>(&self, array: &ArrayView2<F>) -> Result<Vec<Distinct>, ProfilerError>
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
    pub fn compute_bins<F>(
        &self,
        array: &ArrayView1<F>,
        bin_size: &usize,
    ) -> Result<Vec<f64>, ProfilerError>
    where
        F: Float + Num + core::ops::Sub,
        f64: From<F>,
    {
        // find the min and max of the data

        let max: f64 = array
            .max()
            .map_err(|_| ProfilerError::ComputeError("Failed to calculate maximum".to_string()))?
            .to_owned()
            .into();
        let min: f64 = array
            .min()
            .map_err(|_| ProfilerError::ComputeError("Failed to calculate minimum".to_string()))?
            .to_owned()
            .into();

        // create a vector of bins
        let mut bins = Vec::<f64>::with_capacity(*bin_size);

        // compute the bin width
        let bin_width = (max - min) / *bin_size as f64;

        // create the bins
        for i in 0..*bin_size {
            bins.push(min + bin_width * i as f64);
        }

        // return the bins
        Ok(bins)
    }

    pub fn compute_bin_counts<F>(
        &self,
        array: &ArrayView1<F>,
        bins: &[f64],
    ) -> Result<Vec<i32>, ProfilerError>
    where
        F: Num + ndarray_stats::MaybeNan + std::marker::Send + Sync + Clone + Copy,
        f64: From<F>,
    {
        // create a vector of size bins
        let mut bin_counts = vec![0; bins.len()];
        let max_bin = bins.last().ok_or(ProfilerError::ComputeError(
            "Failed to get max bin".to_string(),
        ))?;

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
        bin_size: &usize,
    ) -> Result<HashMap<String, Histogram>, ProfilerError>
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
                    .compute_bins(&x, bin_size)
                    .map_err(|_| {
                        ProfilerError::ComputeError("Failed to calculate bins".to_string())
                    })
                    .unwrap();
                let bin_counts = self
                    .compute_bin_counts(&x, &bins)
                    .map_err(|_| {
                        ProfilerError::ComputeError("Failed to calculate bin counts".to_string())
                    })
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
        bin_size: &usize,
    ) -> Result<Vec<FeatureProfile>, ProfilerError>
    where
        F: Float
            + MaybeNan
            + FromPrimitive
            + std::fmt::Display
            + Sync
            + Send
            + Num
            + Clone
            + std::fmt::Debug
            + 'static,
        F: Into<f64>,
        <F as MaybeNan>::NotNan: Ord,
        f64: From<F>,
        <F as MaybeNan>::NotNan: Clone,
    {
        let means = self
            .compute_mean(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing mean".to_string()))?;

        let stddevs = self
            .compute_stddev(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing standard dev".to_string()))?;
        let quantiles = self
            .compute_quantiles(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing quantiles".to_string()))?;
        let mins = self
            .compute_min(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing minimum".to_string()))?;
        let maxs = self
            .compute_max(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing maximum".to_string()))?;
        let distinct = self
            .compute_distinct(array)
            .map_err(|_| ProfilerError::ComputeError("Error computing distinct".to_string()))?;

        let hist = self
            .compute_histogram(array, features, bin_size)
            .map_err(|_| ProfilerError::ComputeError("Error computing histogram".to_string()))?;

        // loop over list
        let mut profiles = Vec::new();
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

            let numeric_stats = NumericStats {
                mean: f64::from(*mean),
                stddev: f64::from(*stddev),
                min: f64::from(*min),
                max: f64::from(*max),

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

            let profile = FeatureProfile {
                id: features[i].clone(),
                numeric_stats: Some(numeric_stats),
                string_stats: None,
                timestamp: chrono::Utc::now().naive_utc(),
            };

            profiles.push(profile);
        }

        Ok(profiles)
    }

    pub fn process_num_array<F>(
        &mut self,
        compute_correlations: bool,
        numeric_array: &ArrayView2<F>,
        numeric_features: Vec<String>,
        bin_size: Option<usize>,
    ) -> Result<DataProfile, ProfilerError>
    where
        F: Float
            + MaybeNan
            + FromPrimitive
            + std::fmt::Display
            + Sync
            + Send
            + Num
            + Clone
            + std::fmt::Debug
            + 'static,
        F: Into<f64>,
        <F as MaybeNan>::NotNan: Ord,
        f64: From<F>,
        <F as MaybeNan>::NotNan: Clone,
    {
        let profiles = self
            .compute_stats(&numeric_features, numeric_array, &bin_size.unwrap_or(20))
            .map_err(|e| {
                ProfilerError::ComputeError(format!("Failed to create feature data profile: {}", e))
            })?;

        let correlations = if compute_correlations {
            let feature_names = numeric_features.clone();
            let feature_correlations = compute_feature_correlations(numeric_array, &feature_names);

            // convert all values to f64

            Some(feature_correlations)
        } else {
            None
        };

        let features: BTreeMap<String, FeatureProfile> = profiles
            .iter()
            .map(|profile| (profile.id.clone(), profile.clone()))
            .collect();

        Ok(DataProfile {
            features,
            correlations,
        })
    }
}

impl Default for NumProfiler {
    fn default() -> Self {
        NumProfiler::new()
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

        let profiler = NumProfiler::default();
        let bin_size = 20;

        let profile = profiler
            .compute_stats(&features, &array.view(), &bin_size)
            .unwrap();

        assert_eq!(profile.len(), 3);
        assert_eq!(profile[0].id, "feature_1");
        assert_eq!(profile[1].id, "feature_2");
        assert_eq!(profile[2].id, "feature_3");

        // check mean
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().mean,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[1].numeric_stats.as_ref().unwrap().mean,
            1.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[2].numeric_stats.as_ref().unwrap().mean,
            2.5,
            epsilon = 0.05
        ));

        // check quantiles
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q25,
            0.25,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q50,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q75,
            0.75,
            epsilon = 0.1
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q99,
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
        let bin_size = 20;

        let profiler = NumProfiler::default();

        let profile = profiler
            .compute_stats(&features, &array.view(), &bin_size)
            .unwrap();

        assert_eq!(profile.len(), 3);
        assert_eq!(profile[0].id, "feature_1");
        assert_eq!(profile[1].id, "feature_2");
        assert_eq!(profile[2].id, "feature_3");

        // check mean
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().mean,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[1].numeric_stats.as_ref().unwrap().mean,
            1.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[2].numeric_stats.as_ref().unwrap().mean,
            2.5,
            epsilon = 0.05
        ));

        // check quantiles
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q25,
            0.25,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q50,
            0.5,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q75,
            0.75,
            epsilon = 0.05
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q99,
            0.99,
            epsilon = 0.05
        ));
    }
}
