use crate::binning::equal_width::EqualWidthBinning;
use crate::binning::quantile::QuantileBinning;
use crate::error::TypeError;
use ndarray::{Array1, ArrayView1};
use num_traits::{Float, FromPrimitive};
use pyo3::{pyclass, Bound, IntoPyObjectExt, PyAny, PyResult, Python};
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum BinningStrategy {
    QuantileBinning(QuantileBinning),
    EqualWidthBinning(EqualWidthBinning),
}

impl Default for BinningStrategy {
    fn default() -> Self {
        BinningStrategy::QuantileBinning(QuantileBinning::default())
    }
}

impl BinningStrategy {
    pub fn strategy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            BinningStrategy::QuantileBinning(strategy) => strategy.clone().into_bound_py_any(py),
            BinningStrategy::EqualWidthBinning(strategy) => strategy.clone().into_bound_py_any(py),
        }
    }

    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, TypeError>
    where
        F: Float + FromPrimitive,
    {
        let clean_arr = Array1::from(
            arr.iter()
                .filter(|&&x| x.is_finite())
                .cloned()
                .collect::<Vec<F>>(),
        );

        if clean_arr.is_empty() {
            return Err(TypeError::EmptyArrayError(
                "unable to compute bin edges".to_string(),
            ));
        }

        match self {
            BinningStrategy::QuantileBinning(b) => b.compute_edges(arr),
            BinningStrategy::EqualWidthBinning(b) => b.compute_edges(arr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    #[test]
    fn test_empty_array() {
        let binning_strategy = BinningStrategy::QuantileBinning(QuantileBinning { num_bins: 4 });
        let data = Array1::<f64>::from(vec![]);
        let result = binning_strategy.compute_edges(&data.view());

        assert!(result.is_err());
        match result.unwrap_err() {
            TypeError::EmptyArrayError(msg) => {
                assert_eq!(msg, "unable to compute bin edges");
            }
            _ => panic!("Expected EmptyArrayError"),
        }
    }

    #[test]
    fn test_default_method() {
        let default_method = BinningStrategy::default();
        match default_method {
            BinningStrategy::QuantileBinning(_) => {} // Expected
            _ => panic!("Default should be QuantileBinning"),
        }
    }
}
