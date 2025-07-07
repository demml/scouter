use ndarray::ArrayView1;
use ndarray_stats::QuantileExt;
use num_traits::{Float, FromPrimitive};


pub struct Manual{
    pub num_bins: usize,
}

impl Manual {
    pub fn num_bins(&self) -> usize
    {
        self.num_bins
    }
}


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

pub struct Rice;

impl Rice {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n.powf(1.0/3.0)).ceil() as usize
    }
}

pub struct Doane;

impl Doane {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float + PartialOrd + FromPrimitive,
    {
        let n = arr.len() as f64;
        let m3 = 
    }
}


pub struct Scott;

impl Scott {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize
    where
        F: Float + PartialOrd + FromPrimitive,
    {
        let n = arr.len() as f64;
        
        let std_dev = arr.std(F::from(0.0).unwrap()).to_f64().unwrap();
        
        let bin_width = 3.5 * std_dev * n.powf(-1.0/3.0);

        // Get min/max using ndarray-stats
        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let range = (max_val - min_val).to_f64().unwrap();

        // Number of bins = ceil(range / bin_width)
        (range / bin_width).ceil() as usize
    }
}

pub struct TerrellScott;

impl TerrellScott {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n).powf(1.0/3.0).ceil() as usize
    }
}


pub enum EqualWidthMethod {
    Manual(usize),
    SquareRoot(SquareRoot),
    Sturges(Sturges),
    Rice(Rice),
    Doane(Doane),
    Scott(Scott),
    TerrellScott(TerrellScott)
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
            EqualWidthMethod::Rice(m) => m.num_bins(arr),
            EqualWidthMethod::Scott(m) => m.num_bins(arr),
            EqualWidthMethod::TerrellScott(m) => m.num_bins(arr),
            _ => panic!()
        }
    }
}

pub struct EqualWidthBinning {
    pub method: EqualWidthMethod,
}

impl EqualWidthBinning {

}