extern crate blas_src;

mod logging;
mod math;
mod profiler;
mod types;
use math::stats::{compute_2d_array_stats, compute_mean_test};
use ndarray::prelude::*;
use numpy::PyReadonlyArray2;
use pyo3::panic::PanicException;
use pyo3::prelude::*;
use rayon::prelude::*;
use types::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
