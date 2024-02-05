use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::math::logger::Logger;
use crate::math::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
use medians::{MStats, Median, Medianf64};
use ndarray::prelude::*;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::prelude::*;
use noisy_float::types::n64;
use num_traits::Float;
use numpy::ndarray::{aview1, ArrayView1, ArrayView2};
use rayon::prelude::*;
use std::collections::HashSet;

struct DataProfiler<'a> {
    array_data: &'a ArrayView2<'a, f64>,
    feature_names: &'a [String],
}
