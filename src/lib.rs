extern crate blas_src;

mod logging;
mod math;
mod profiler;
mod types;
use ndarray::prelude::*;
use numpy::PyReadonlyArray2;
use pyo3::panic::PanicException;
use pyo3::prelude::*;
use rayon::prelude::*;
use types::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
