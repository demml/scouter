use ndarray::ArrayView1;
use ndarray_stats::QuantileExt;
use num_traits::{Float, FromPrimitive};
use crate::error::DriftError;

pub struct SquareRoot;

impl SquareRoot {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    {
        let n = arr.len() as f64;
        n.sqrt().ceil() as usize
    }
}

pub struct Sturges;

impl Sturges {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    {
        let n = arr.len() as f64;
        (n.log2().ceil() + 1.0) as usize
    }
}
//
// pub struct Rice;
//
// impl Rice {
//     pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
//         let n = arr.len() as f64;
//         (2.0 * n.powf(1.0/3.0)).ceil() as usize
//     }
// }
//
// pub struct Doane;
//
// impl Doane {
//     pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
//     where
//         F: Float + PartialOrd + FromPrimitive,
//     {
//         let n = arr.len() as f64;
//         let m3 =
//     }
// }
//
//
// pub struct Scott;
//
// impl Scott {
//     pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
//     where
//         F: Float + PartialOrd + FromPrimitive,
//     {
//         let n = arr.len() as f64;
//
//         let std_dev = arr.std(F::from(0.0).unwrap()).to_f64().unwrap();
//
//         let bin_width = 3.5 * std_dev * n.powf(-1.0/3.0);
//
//         // Get min/max using ndarray-stats
//         let min_val = *arr.min().unwrap();
//         let max_val = *arr.max().unwrap();
//         let range = (max_val - min_val).to_f64().unwrap();
//
//         // Number of bins = ceil(range / bin_width)
//         (range / bin_width).ceil() as usize
//     }
// }
//
// pub struct TerrellScott;
//
// impl TerrellScott {
//     pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
//         let n = arr.len() as f64;
//         (2.0 * n).powf(1.0/3.0).ceil() as usize
//     }
// }


pub enum EqualWidthMethod {
    Manual(usize),
    SquareRoot(SquareRoot),
    Sturges(Sturges),
    // Rice(Rice),
    // Doane(Doane),
    // Scott(Scott),
    // TerrellScott(TerrellScott)
}

impl EqualWidthMethod {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float + Default + Copy + PartialOrd + Clone + FromPrimitive,
    {
        match &self {
            EqualWidthMethod::Manual(n) => *n,
            EqualWidthMethod::SquareRoot(m) => m.num_bins(arr),
            EqualWidthMethod::Sturges(m) => m.num_bins(arr),
            // EqualWidthMethod::Rice(m) => m.num_bins(arr),
            // EqualWidthMethod::Scott(m) => m.num_bins(arr),
            // EqualWidthMethod::TerrellScott(m) => m.num_bins(arr),
            _ => panic!()
        }
    }
}

pub struct EqualWidthBinning {
    pub method: EqualWidthMethod,
}

impl EqualWidthBinning {
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, DriftError>
    where
        F: Float + Default + Copy + PartialOrd + Clone + FromPrimitive,
    {
        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let num_bins = self.method.num_bins(arr);

        let range = max_val - min_val;
        let bin_width = range / F::from_usize(num_bins).unwrap();

        Ok((0..=num_bins)
            .map(|i| min_val + bin_width * F::from_usize(i).unwrap())
            .collect())
    }
}


#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use super::*;
    use ndarray::{Array, Array1};
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;


    #[test]
    fn test_compute_edges_0_to_100_manual_10_bins() {
        let equal_width_manual = EqualWidthBinning {
            method: EqualWidthMethod::Sturges(Sturges),
        };

        // Create array with values 0 to 100
        let data: Vec<f64> = (0..=100).map(|i| i as f64).collect();
        let array = Array1::from(data);

        let edges = equal_width_manual.compute_edges(&array.view()).unwrap();
        println!("{:?}", edges);
    }
}