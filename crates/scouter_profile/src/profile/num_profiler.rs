use crate::error::DataProfileError;
use crate::profile::stats::compute_feature_correlations;
use crate::profile::types::DataProfile;
use crate::profile::types::{Distinct, FeatureProfile, Histogram, NumericStats, Quantiles};
use ndarray::prelude::*;
use ndarray::{aview1, Axis};
use ndarray_stats::MaybeNan;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::types::{n64, N64};
use num_traits::ToPrimitive;
use num_traits::{Float, FromPrimitive, Num};
use rayon::prelude::*;
use std::cmp::Ord;
use std::collections::{BTreeMap, HashMap, HashSet};
use tracing::{debug, error, warn};
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
    pub fn compute_quantiles<F>(
        &self,
        array: &ArrayView2<F>,
    ) -> Result<(Option<Array2<N64>>, bool), DataProfileError>
    where
        F: Num + ndarray_stats::MaybeNan + std::marker::Send + Sync + Clone + Copy + Float,
        <F as ndarray_stats::MaybeNan>::NotNan: Clone,
        <F as ndarray_stats::MaybeNan>::NotNan: Ord,
        f64: From<F>,
    {
        // First convert to f64, then to n64
        // Check for NaN or Inf values early to avoid unnecessary computation
        if array.iter().any(|&x| x.is_nan() || x.is_infinite()) {
            warn!("Array contains NaN or Inf values, skipping quantile computation");
            return Ok((None, true));
        }

        // Convert F values to n64 in one step
        let mut n64_array = array.mapv(|x| n64(f64::from(x)));

        let qs = &[n64(0.25), n64(0.5), n64(0.75), n64(0.99)];
        let quantiles = n64_array.quantiles_axis_mut(Axis(0), &aview1(qs), &Nearest)?;

        Ok((Some(quantiles), false))
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
    pub fn compute_mean<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, DataProfileError>
    where
        F: FromPrimitive + Num + Clone,
    {
        let mean = array
            .mean_axis(Axis(0))
            .ok_or(DataProfileError::MeanError)?;

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
    pub fn compute_stddev<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, DataProfileError>
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
    pub fn compute_min<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, DataProfileError>
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
    pub fn compute_max<F>(&self, array: &ArrayView2<F>) -> Result<Array1<F>, DataProfileError>
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
    pub fn compute_distinct<F>(
        &self,
        array: &ArrayView2<F>,
    ) -> Result<Vec<Distinct>, DataProfileError>
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
    ) -> Result<Vec<f64>, DataProfileError>
    where
        F: Float + Num + core::ops::Sub,
        f64: From<F>,
    {
        // find the min and max of the data

        let max: f64 = array.max()?.to_owned().into();
        let min: f64 = array.min()?.to_owned().into();

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
    ) -> Result<Vec<i32>, DataProfileError>
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
        // create a vector of size bins
        let mut bin_counts = vec![0; bins.len()];
        let max_bin = bins.last().ok_or(DataProfileError::MaxBinError)?;

        array.for_each(|datum| {
            let val: f64 = datum.to_owned().into();

            // iterate over the bins
            for (i, bin) in bins.iter().enumerate() {
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
        has_unsupported_types: bool,
    ) -> Result<HashMap<String, Histogram>, DataProfileError>
    where
        F: Num
            + ndarray_stats::MaybeNan
            + std::marker::Send
            + Sync
            + Clone
            + Copy
            + num_traits::Float
            + std::fmt::Debug,
        f64: From<F>,
    {
        // Process each column in parallel
        array
            .axis_iter(Axis(1))
            .into_par_iter()
            .enumerate()
            .map(|(idx, column)| {
                // Compute histogram components

                if has_unsupported_types {
                    warn!(
                        "Skipping histogram computation for feature {} due to unsupported types",
                        features.get(idx).unwrap_or(&"Unknown".to_string())
                    );
                    return Ok((features[idx].clone(), Histogram::default()));
                }

                let bins = self.compute_bins(&column, bin_size).map_err(|e| {
                    error!(
                        error = %e,
                        feature = %features.get(idx).unwrap_or(&"Unknown".to_string()),
                        column = ?column,
                        bin_size = bin_size,
                        "Failed to compute bins"
                    );
                    e
                })?;
                let bin_counts = self.compute_bin_counts(&column, &bins).map_err(|e| {
                    error!(
                        error = %e,
                        feature = %features.get(idx).unwrap_or(&"Unknown".to_string()),
                        "Failed to compute bin counts"
                    );
                    e
                })?;

                // Create histogram for this feature
                Ok((features[idx].clone(), Histogram { bins, bin_counts }))
            })
            .collect()
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
    ) -> Result<Vec<FeatureProfile>, DataProfileError>
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
        let means = self.compute_mean(array)?;

        debug!("Computing stddev");
        let stddevs = self.compute_stddev(array)?;

        debug!("Computing quantiles");
        let (quantiles, has_unsupported_types) = self.compute_quantiles(array)?;

        debug!("Computing min");
        let mins = self.compute_min(array)?;

        debug!("Computing max");
        let maxs = self.compute_max(array)?;

        debug!("Computing distinct values");
        let distinct = self.compute_distinct(array)?;

        debug!("Computing histogram");
        let hist = self.compute_histogram(array, features, bin_size, has_unsupported_types)?;

        // loop over list
        let mut profiles = Vec::new();
        for i in 0..features.len() {
            let mean = &means[i];
            let stddev = &stddevs[i];
            let min = &mins[i];
            let max = &maxs[i];
            let q25 = quantiles.as_ref().map(|q| q[[0, i]]);
            let q50 = quantiles.as_ref().map(|q| q[[1, i]]);
            let q75 = quantiles.as_ref().map(|q| q[[2, i]]);
            let q99 = quantiles.as_ref().map(|q| q[[3, i]]);
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
                    q25: q25.unwrap_or_default().to_f64().unwrap_or_default(),
                    q50: q50.unwrap_or_default().to_f64().unwrap_or_default(),
                    q75: q75.unwrap_or_default().to_f64().unwrap_or_default(),
                    q99: q99.unwrap_or_default().to_f64().unwrap_or_default(),
                },
                histogram: hist[&features[i]].clone(),
            };

            let profile = FeatureProfile {
                id: features[i].clone(),
                numeric_stats: Some(numeric_stats),
                string_stats: None,
                timestamp: chrono::Utc::now(),
                correlations: None,
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
        bin_size: usize,
    ) -> Result<DataProfile, DataProfileError>
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
        let profiles = self.compute_stats(&numeric_features, numeric_array, &bin_size)?;
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
            .map(|profile| {
                let mut profile = profile.clone();

                if let Some(correlations) = correlations.as_ref() {
                    let correlation = correlations.get(&profile.id);
                    if let Some(correlation) = correlation {
                        profile.add_correlations(correlation.clone());
                    }
                }

                (profile.id.clone(), profile)
            })
            .collect();

        Ok(DataProfile { features })
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
            epsilon = 0.1
        ));
        assert!(relative_eq!(
            profile[1].numeric_stats.as_ref().unwrap().mean,
            1.5,
            epsilon = 0.1
        ));
        assert!(relative_eq!(
            profile[2].numeric_stats.as_ref().unwrap().mean,
            2.5,
            epsilon = 0.1
        ));

        // check quantiles
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q25,
            0.25,
            epsilon = 0.1
        ));

        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q50,
            0.5,
            epsilon = 0.1
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q75,
            0.75,
            epsilon = 0.1
        ));
        assert!(relative_eq!(
            profile[0].numeric_stats.as_ref().unwrap().quantiles.q99,
            0.99,
            epsilon = 0.1
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
