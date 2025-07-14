use crate::error::DriftError;
use ndarray::ArrayView1;
use num_traits::{Float, FromPrimitive};

pub struct QuantileBinning {
    pub num_quantiles: usize,
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
    /// This method is the default in R and provides continuous, median-unbiased
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
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, DriftError>
    where
        F: Float + Default + Copy + PartialOrd + Clone + FromPrimitive,
    {
        if arr.len() < self.num_quantiles {
            return Err(DriftError::InsufficientDataError(format!(
                "Need at least {} data points for {} quantiles, got {}",
                self.num_quantiles,
                self.num_quantiles,
                arr.len()
            )));
        }

        if self.num_quantiles < 2 {
            return Err(DriftError::InvalidParameterError(
                "num_quantiles must be at least 2".to_string(),
            ));
        }

        let mut data: Vec<F> = arr.to_vec();
        data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let mut edges = Vec::new();
        let n = data.len();

        for i in 1..self.num_quantiles {
            let p = i as f64 / self.num_quantiles as f64;

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
