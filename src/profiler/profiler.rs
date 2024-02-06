use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::math::stats::{
    compute_max, compute_mean, compute_min, compute_quantiles, compute_stddev,
};
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

struct DataProfiler<'a> {
    array_data: &'a ArrayView2<'a, f64>,
    feature_names: &'a [String],
}

impl<'a> DataProfiler<'a> {
    pub fn new(
        array_data: &'a ArrayView2<'a, f64>,
        feature_names: &'a [String],
    ) -> DataProfiler<'a> {
        DataProfiler {
            array_data: array_data,
            feature_names: feature_names,
        }
    }

    pub fn compute_profile(&self) -> Result<()> {
        info!("Computing data stats");
        let funcs = [compute_max, compute_min, compute_mean, compute_stddev];
        let base_stats = funcs
            .par_iter()
            .map(|func| func(&self.array_data))
            .collect::<Vec<_>>();
        let quantiles = compute_quantiles(&self.array_data);
        Ok(())
    }
}
