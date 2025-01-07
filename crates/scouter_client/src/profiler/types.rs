use numpy::PyReadonlyArray2;

pub type ConvertedArray<'py> = (
    Vec<String>,
    Option<PyReadonlyArray2<'py, f64>>,
    Vec<String>,
    Option<Vec<Vec<String>>>,
);
