use crate::error::TypeError;
use ndarray::ArrayView1;
use num_traits::{Float, FromPrimitive};
use pyo3::{pyclass, pymethods, PyResult};
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct QuantileBinning {
    #[pyo3(get, set)]
    pub num_bins: usize,
}

#[pymethods]
impl QuantileBinning {
    #[new]
    #[pyo3(signature = (num_bins=10))]
    pub fn new(num_bins: usize) -> PyResult<Self> {
        Ok(QuantileBinning { num_bins })
    }
}

impl Default for QuantileBinning {
    fn default() -> Self {
        QuantileBinning { num_bins: 10 }
    }
}

impl QuantileBinning {
    /// Computes quantile edges for binning using the R-7 method (Hyndman & Fan Type 7).
    ///
    /// This implementation follows the R-7 quantile definition from:
    /// Hyndman, R. J. and Fan, Y. (1996) "Sample quantiles in statistical packages,"
    /// The American Statistician, 50(4), pp. 361-365.
    ///
    /// The R-7 method uses the formula:
    /// - m = 1 - p
    /// - j = floor(np + m)
    /// - h = np + m - j
    /// - Q(p) = (1 - h) × x[j] + h × x[j+1]
    ///
    /// This method is the default in many statistical packages, median-unbiased
    /// quantile estimates that are approximately unbiased for normal distributions.
    ///
    /// # Arguments
    /// * `arr` - Sorted array of data values
    ///
    /// # Returns
    /// * `Ok(Vec<F>)` - Vector of quantile edge values for binning
    /// * `Err(DriftError)` - If insufficient data points for quantile calculation
    ///
    /// # Reference
    /// PDF: https://www.amherst.edu/media/view/129116/original/Sample+Quantiles.pdf
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, TypeError>
    where
        F: Float + FromPrimitive,
    {
        if self.num_bins < 2 {
            return Err(TypeError::InvalidParameterError(
                "num_bins must be at least 2".to_string(),
            ));
        }

        let mut data: Vec<F> = arr.to_vec();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut edges = Vec::new();
        let n = data.len();

        for i in 1..self.num_bins {
            let p = i as f64 / self.num_bins as f64;

            // R-7 Formula Implementation
            // Step 1: Calculate m (R-7 parameter: m = 1 - p)
            let m = 1.0 - p;

            // Step 2: Calculate np + m
            let np_plus_m = (n as f64) * p + m;

            // Step 3: Calculate j = floor(np + m)
            let j = np_plus_m.floor() as usize;

            // Step 4: Calculate h = np + m - j (fractional part)
            let h = np_plus_m - (j as f64);

            // Step 5: Convert j from 1-indexed (paper) to 0-indexed
            let j_zero_indexed = if j > 0 { j - 1 } else { 0 };
            let j_plus_1_zero_indexed = std::cmp::min(j_zero_indexed + 1, n - 1);

            // Step 6: Apply R-7 interpolation formula
            // Q(p) = (1 - h) × x[j] + h × x[j+1]
            let one_minus_h = F::from_f64(1.0 - h).unwrap();
            let h_f = F::from_f64(h).unwrap();

            let quantile = one_minus_h * data[j_zero_indexed] + h_f * data[j_plus_1_zero_indexed];

            edges.push(quantile);
        }

        Ok(edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use ndarray::Array1;

    #[test]
    fn test_invalid_num_bins() {
        let binning = QuantileBinning { num_bins: 1 };
        let data = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        let result = binning.compute_edges(&data.view());

        assert!(result.is_err());
        match result.unwrap_err() {
            TypeError::InvalidParameterError(msg) => {
                assert_eq!(msg, "num_bins must be at least 2");
            }
            _ => panic!("Expected InvalidParameterError"),
        }
    }

    #[test]
    fn test_quartiles_simple_case() {
        let binning = QuantileBinning { num_bins: 4 };
        let data = Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let edges = binning.compute_edges(&data.view()).unwrap();

        assert_eq!(edges.len(), 3); // 4 quantiles = 3 edges

        // For R-7 method with 8 data points and quartiles:
        // Q1 (p=0.25): should be around 2.75
        // Q2 (p=0.50): should be around 4.5
        // Q3 (p=0.75): should be around 6.25
        assert_abs_diff_eq!(edges[0], 2.75, epsilon = 1e-10);
        assert_abs_diff_eq!(edges[1], 4.5, epsilon = 1e-10);
        assert_abs_diff_eq!(edges[2], 6.25, epsilon = 1e-10);
    }

    #[test]
    fn test_unsorted_data_produces_monotonic_edges() {
        let binning = QuantileBinning { num_bins: 5 };
        // Deliberately unsorted data
        let data = Array1::from(vec![
            12.0, 8.0, 17.0, 33.0, 123.0, 6.0, 9.23, 123.43, 1.9, 4.0, 11.0, 2.0, 5.6,
        ]);
        let edges = binning.compute_edges(&data.view()).unwrap();

        assert_eq!(edges.len(), 4);

        // Verify that quantile edges are monotonically increasing despite unsorted input
        for i in 1..edges.len() {
            assert!(edges[i] > edges[i-1],
                    "Quantile edges should be monotonically increasing even with unsorted input: {} > {}",
                    edges[i], edges[i-1]);
        }
    }
}
