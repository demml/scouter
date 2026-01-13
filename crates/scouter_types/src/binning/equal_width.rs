use crate::error::TypeError;
use ndarray::ArrayView1;
use ndarray_stats::QuantileExt;
use num_traits::{Float, FromPrimitive};
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyclass, pymethods, Bound, IntoPyObjectExt, PyAny, PyResult, Python};
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Manual {
    #[pyo3(get, set)]
    num_bins: usize,
}

#[pymethods]
impl Manual {
    #[new]
    pub fn new(num_bins: usize) -> Self {
        Manual { num_bins }
    }
}

impl Manual {
    pub fn num_bins(&self) -> usize {
        self.num_bins
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SquareRoot;

impl Default for SquareRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl SquareRoot {
    #[new]
    pub fn new() -> Self {
        SquareRoot
    }
}

impl SquareRoot {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        n.sqrt().ceil() as usize
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Sturges;

#[pymethods]
impl Sturges {
    #[new]
    pub fn new() -> Self {
        Sturges
    }
}

impl Default for Sturges {
    fn default() -> Self {
        Self::new()
    }
}

impl Sturges {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (n.log2().ceil() + 1.0) as usize
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Rice;

#[pymethods]
impl Rice {
    #[new]
    pub fn new() -> Self {
        Rice
    }
}

impl Default for Rice {
    fn default() -> Self {
        Self::new()
    }
}

impl Rice {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n.powf(1.0 / 3.0)).ceil() as usize
    }
}
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Doane;

#[pymethods]
impl Doane {
    #[new]
    pub fn new() -> Self {
        Doane
    }
}

impl Default for Doane {
    fn default() -> Self {
        Self::new()
    }
}

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
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Scott;

#[pymethods]
impl Scott {
    #[new]
    pub fn new() -> Self {
        Scott
    }
}

impl Default for Scott {
    fn default() -> Self {
        Self::new()
    }
}

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
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TerrellScott;

#[pymethods]
impl TerrellScott {
    #[new]
    pub fn new() -> Self {
        TerrellScott
    }
}

impl Default for TerrellScott {
    fn default() -> Self {
        Self::new()
    }
}

impl TerrellScott {
    pub fn num_bins<F>(&self, arr: &ArrayView1<F>) -> usize {
        let n = arr.len() as f64;
        (2.0 * n).powf(1.0 / 3.0).round() as usize
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FreedmanDiaconis;

impl Default for FreedmanDiaconis {
    fn default() -> Self {
        Self::new()
    }
}

#[pymethods]
impl FreedmanDiaconis {
    #[new]
    pub fn new() -> Self {
        FreedmanDiaconis
    }
}

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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum EqualWidthMethod {
    Manual(Manual),
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
            EqualWidthMethod::Manual(m) => m.num_bins(),
            EqualWidthMethod::SquareRoot(m) => m.num_bins(arr),
            EqualWidthMethod::Sturges(m) => m.num_bins(arr),
            EqualWidthMethod::Rice(m) => m.num_bins(arr),
            EqualWidthMethod::Doane(m) => m.num_bins(arr),
            EqualWidthMethod::Scott(m) => m.num_bins(arr),
            EqualWidthMethod::TerrellScott(m) => m.num_bins(arr),
            EqualWidthMethod::FreedmanDiaconis(m) => m.num_bins(arr),
        }
    }
}

impl Default for EqualWidthMethod {
    fn default() -> Self {
        EqualWidthMethod::Doane(Doane)
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct EqualWidthBinning {
    pub method: EqualWidthMethod,
}

#[pymethods]
impl EqualWidthBinning {
    #[new]
    #[pyo3(signature = (method=None))]
    pub fn new(method: Option<&Bound<'_, PyAny>>) -> Result<Self, TypeError> {
        let method = match method {
            None => EqualWidthMethod::default(),
            Some(method_obj) => {
                if method_obj.is_instance_of::<Manual>() {
                    EqualWidthMethod::Manual(method_obj.extract()?)
                } else if method_obj.is_instance_of::<SquareRoot>() {
                    EqualWidthMethod::SquareRoot(method_obj.extract()?)
                } else if method_obj.is_instance_of::<Rice>() {
                    EqualWidthMethod::Rice(method_obj.extract()?)
                } else if method_obj.is_instance_of::<Sturges>() {
                    EqualWidthMethod::Sturges(method_obj.extract()?)
                } else if method_obj.is_instance_of::<Doane>() {
                    EqualWidthMethod::Doane(method_obj.extract()?)
                } else if method_obj.is_instance_of::<Scott>() {
                    EqualWidthMethod::Scott(method_obj.extract()?)
                } else if method_obj.is_instance_of::<TerrellScott>() {
                    EqualWidthMethod::TerrellScott(method_obj.extract()?)
                } else if method_obj.is_instance_of::<FreedmanDiaconis>() {
                    EqualWidthMethod::FreedmanDiaconis(method_obj.extract()?)
                } else {
                    return Err(TypeError::InvalidEqualWidthBinningMethodError);
                }
            }
        };

        Ok(EqualWidthBinning { method })
    }

    #[getter]
    pub fn method<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.method {
            EqualWidthMethod::Manual(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::SquareRoot(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::Sturges(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::Rice(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::Doane(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::Scott(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::TerrellScott(m) => m.clone().into_bound_py_any(py),
            EqualWidthMethod::FreedmanDiaconis(m) => m.clone().into_bound_py_any(py),
        }
    }
}

impl EqualWidthBinning {
    pub fn compute_edges<F>(&self, arr: &ArrayView1<F>) -> Result<Vec<F>, TypeError>
    where
        F: Float + FromPrimitive,
    {
        let min_val = *arr.min().unwrap();
        let max_val = *arr.max().unwrap();
        let num_bins = self.method.num_bins(arr);

        if num_bins < 2 {
            return Err(TypeError::InvalidBinCountError(
                format!("Specified Binning strategy did not return enough bins, at least 2 are needed, got {num_bins}")
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
    use ndarray::{arr1, Array1};
    use ndarray_rand::rand_distr::Normal;
    use ndarray_rand::RandomExt;
    use test_utils::retry_flaky_test_sync;

    fn create_normal_data(n: usize, mean: f64, std: f64) -> Array1<f64> {
        Array1::random(n, Normal::new(mean, std).unwrap())
    }

    #[test]
    fn test_manual_basic() {
        let manual = Manual::new(10);
        assert_eq!(manual.num_bins(), 10);
        assert_eq!(manual.num_bins, 10);
    }

    // SquareRoot method tests
    #[test]
    fn test_square_root_known_values() {
        let sr = SquareRoot::new();

        // Perfect squares
        let arr = arr1(&[1.0; 9]);
        assert_eq!(sr.num_bins(&arr.view()), 3);

        let arr = arr1(&[1.0; 100]);
        assert_eq!(sr.num_bins(&arr.view()), 10);

        let arr = arr1(&[1.0; 64]);
        assert_eq!(sr.num_bins(&arr.view()), 8);
    }

    #[test]
    fn test_square_root_non_perfect_squares() {
        let sr = SquareRoot::new();

        let arr = arr1(&[1.0; 10]);
        assert_eq!(sr.num_bins(&arr.view()), 4); // ceil(sqrt(10)) = 4

        let arr = arr1(&[1.0; 50]);
        assert_eq!(sr.num_bins(&arr.view()), 8); // ceil(sqrt(50)) = 8
    }

    // Sturges method tests
    #[test]
    fn test_sturges_known_values() {
        let sturges = Sturges::new();

        let arr = arr1(&[1.0; 16]);
        assert_eq!(sturges.num_bins(&arr.view()), 5); // log2(16) + 1 = 5

        let arr = arr1(&[1.0; 32]);
        assert_eq!(sturges.num_bins(&arr.view()), 6); // log2(32) + 1 = 6

        let arr = arr1(&[1.0; 128]);
        assert_eq!(sturges.num_bins(&arr.view()), 8); // log2(128) + 1 = 8
    }

    #[test]
    fn test_scott_different_scales() {
        retry_flaky_test_sync!(3, 1000, {
            let scott = Scott::new();

            // Same distribution shape but different scales
            let arr1 = create_normal_data(100, 0.0, 1.0);
            let arr2 = create_normal_data(100, 0.0, 10.0);

            let bins1 = scott.num_bins(&arr1.view());
            let bins2 = scott.num_bins(&arr2.view());

            // Both should give similar bin counts since Scott's rule accounts for scale
            assert!((bins1 as i32 - bins2 as i32).abs() <= 2);
        });
    }

    #[test]
    fn test_terrell_scott_known_values() {
        let ts = TerrellScott::new();

        let arr = arr1(&[1.0; 8]);
        assert_eq!(ts.num_bins(&arr.view()), 3); // round((2*8)^(1/3)) = 3

        let arr = arr1(&[1.0; 125]);
        assert_eq!(ts.num_bins(&arr.view()), 6); // round((2*125)^(1/3)) = 6
    }

    #[test]
    fn test_freedman_diaconis_heavy_tailed() {
        retry_flaky_test_sync!(3, 1000, {
            let fd = FreedmanDiaconis::new();
            // Test with heavy-tailed distribution (using scaled normal as approximation)
            let mut arr = create_normal_data(200, 0.0, 3.0);
            // Add some extreme values to create heavy tails
            for i in 0..10 {
                arr[i] *= 3.0
            }

            let bins = fd.num_bins(&arr.view());
            assert!(
                bins > 3 && bins < 30,
                "Expected bins between 3 and 30, got {}",
                bins
            );
        });
    }

    #[test]
    fn test_small_arrays() {
        let arr = arr1(&[1.0, 2.0, 3.0]);

        assert_eq!(SquareRoot::new().num_bins(&arr.view()), 2);
        assert_eq!(Sturges::new().num_bins(&arr.view()), 3);
        assert_eq!(Rice::new().num_bins(&arr.view()), 3);

        let doane_bins = Doane::new().num_bins(&arr.view());
        assert!((1..=5).contains(&doane_bins));
    }

    #[test]
    fn test_default_method() {
        let default_method = EqualWidthMethod::default();
        match default_method {
            EqualWidthMethod::Doane(_) => {} // Expected
            _ => panic!("Default should be Doane method"),
        }
    }

    #[test]
    fn test_equal_width_method_serialization() {
        let methods = vec![
            EqualWidthMethod::Manual(Manual::new(10)),
            EqualWidthMethod::SquareRoot(SquareRoot::new()),
            EqualWidthMethod::Sturges(Sturges::new()),
            EqualWidthMethod::Rice(Rice::new()),
            EqualWidthMethod::Doane(Doane::new()),
            EqualWidthMethod::Scott(Scott::new()),
            EqualWidthMethod::TerrellScott(TerrellScott::new()),
            EqualWidthMethod::FreedmanDiaconis(FreedmanDiaconis::new()),
        ];

        for method in methods {
            let serialized = serde_json::to_string(&method).unwrap();
            let deserialized: EqualWidthMethod = serde_json::from_str(&serialized).unwrap();
            assert_eq!(method, deserialized);
        }
    }

    #[test]
    fn test_extreme_ranges() {
        let arr = arr1(&[1e-10, 1e10]);

        // Methods should handle extreme ranges without panic
        let _sqrt_bins = SquareRoot::new().num_bins(&arr.view());
        let _sturges_bins = Sturges::new().num_bins(&arr.view());
        let _rice_bins = Rice::new().num_bins(&arr.view());
        let _doane_bins = Doane::new().num_bins(&arr.view());
        let _scott_bins = Scott::new().num_bins(&arr.view());
        let _ts_bins = TerrellScott::new().num_bins(&arr.view());
        let _fd_bins = FreedmanDiaconis::new().num_bins(&arr.view());
    }
}
