use crate::error::DriftError;
use ndarray::ArrayView1;
use ndarray_stats::{QuantileExt};
use num_traits::{Float, FromPrimitive};

pub struct SquareRoot;

impl SquareRoot {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        n.sqrt().ceil() as usize
    }
}

pub struct Sturges;

impl Sturges {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (n.log2().ceil() + 1.0) as usize
    }
}

pub struct Rice;

impl Rice {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n.powf(1.0 / 3.0)).ceil() as usize
    }
}

pub struct Doane;

impl Doane {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float,
    {
        let n = arr.len() as f64;
        let data: Vec<f64> = arr.iter().map(|&x| x.to_f64().unwrap()).collect();
        let mu = data.iter().sum::<f64>() / n;
        let m2 = data.iter().map(|&x| (x - mu).powi(2)).sum::<f64>() / n;
        let m3 = data.iter().map(|&x| (x - mu).powi(3)).sum::<f64>() / n;
        let g1 = m3 / m2.powf(3.0 / 2.0);
        let sigma_g1 = ((6.0 * (n - 2.0)) / ((n + 1.0) * (n + 3.0))).sqrt();
        let k = 1.0 + n.log2() + (1.0 + g1.abs() / sigma_g1).log2();
        k.round() as usize
    }
}

pub struct Scott;

impl Scott {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float + FromPrimitive,
    {
        let n = arr.len() as f64;

        let std_dev = arr.std(F::from(0.0).unwrap()).to_f64().unwrap();

        let bin_width = 3.49 * std_dev * n.powf(-1.0 / 3.0);

        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let range = (max_val - min_val).to_f64().unwrap();

        (range / bin_width).ceil() as usize
    }
}

pub struct TerrellScott;

impl TerrellScott {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n).powf(1.0 / 3.0).ceil() as usize
    }
}

pub struct FreedmanDiaconis;

impl FreedmanDiaconis {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float,
    {
        let mut data: Vec<f64> = arr.iter().map(|&x| x.to_f64().unwrap()).collect();
        let n = data.len() as f64;

        data.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let q1_index = (0.25 * (data.len() - 1) as f64) as usize;
        let q3_index = (0.75 * (data.len() - 1) as f64) as usize;

        let q1 = data[q1_index];
        let q3 = data[q3_index];

        let iqr = q3 - q1;

        let bin_width = 2.0 * iqr / n.powf(1.0 / 3.0);

        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let range = (max_val - min_val).to_f64().unwrap();

        (range / bin_width).ceil() as usize
    }
}

pub enum EqualWidthMethod {
    Manual(usize),
    SquareRoot(SquareRoot),
    Sturges(Sturges),
    Rice(Rice),
    Doane(Doane),
    Scott(Scott),
    TerrellScott(TerrellScott),
    FreedmanDiaconis(FreedmanDiaconis),
}

impl EqualWidthMethod {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float + FromPrimitive,
    {
        match &self {
            EqualWidthMethod::Manual(n) => *n,
            EqualWidthMethod::SquareRoot(m) => m.num_bins(arr),
            EqualWidthMethod::Sturges(m) => m.num_bins(arr),
            EqualWidthMethod::Rice(m) => m.num_bins(arr),
            EqualWidthMethod::Doane(m) => m.num_bins(arr),
            EqualWidthMethod::Scott(m) => m.num_bins(arr),
            EqualWidthMethod::TerrellScott(m) => m.num_bins(arr),
            EqualWidthMethod::FreedmanDiaconis(m) => m.num_bins(arr)
        }
    }
}

pub struct EqualWidthBinning {
    pub method: EqualWidthMethod,
}

impl EqualWidthBinning {
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, DriftError>
    where
        F: Float + FromPrimitive,
    {
        if arr.len() < 2 {
            return Err(DriftError::InsufficientDataError(
                "Need at least 2 data points for equal width binning".to_string(),
            ));
        }

        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let num_bins = self.method.num_bins(arr);

        if num_bins < 2 {
            return Err(DriftError::InvalidBinCountError(
                format!("Specified Binning strategy did not return enough bins, at least 2 are needed, got {}", num_bins)
            ));
        }

        let range = max_val - min_val;
        let bin_width = range / F::from_usize(num_bins).unwrap();

        Ok((1..num_bins)
            .map(|i| min_val + bin_width * F::from_usize(i).unwrap())
            .collect())
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{Array1, ArrayView1};
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

    #[test]
    fn test_insufficient_data_error() {
        let binning = EqualWidthBinning { method: EqualWidthMethod::Manual(10) };

        // Fix 1: Create a 1D array directly, not 2D
        let array: Array1<f64> = Array1::random(1030, Uniform::new(0., 100.));

        // Fix 2: Just use .view() once, not .view().view()
        let result = binning.compute_edges(&array.view()).unwrap();

        println!("{result:?}")
    }

}