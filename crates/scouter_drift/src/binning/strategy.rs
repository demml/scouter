use ndarray::ArrayView1;
use num_traits::{Float, FromPrimitive};
use crate::binning::equal_width::EqualWidthBinning;
use crate::binning::quantile::QuantileBinning;
use crate::error::DriftError;

pub enum BinningStrategy {
    QuantileBinning(QuantileBinning),
    EqualWidthBinning(EqualWidthBinning)
}

impl BinningStrategy {
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, DriftError>
    where
        F: Float + FromPrimitive,
    {
        if arr.iter().any(|&x| x.is_nan() || x.is_infinite()) {
            return Err(DriftError::InvalidValueError(
                "nan or infinity values detected in your profile data, unable to compute bin edges.".to_string(),
            ));
        }

        match self {
            BinningStrategy::QuantileBinning(b) => b.compute_edges(arr),
            BinningStrategy::EqualWidthBinning(b) => b.compute_edges(arr),
        }
    }
}