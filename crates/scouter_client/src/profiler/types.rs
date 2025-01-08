use pyo3::prelude::*;

pub type ConvertedArray<'py> = (
    Vec<String>,
    Option<Bound<'py, PyAny>>,
    Option<String>,
    Vec<String>,
    Option<Vec<Vec<String>>>,
);
