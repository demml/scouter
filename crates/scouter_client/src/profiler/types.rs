use numpy::PyReadonlyArray2;

pub type ConvertedArray<'py, F> = (
    Vec<String>,
    Option<PyReadonlyArray2<'py, F>>,
    Vec<String>,
    Option<Vec<Vec<String>>>,
);
