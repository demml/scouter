use pyo3::prelude::*;

pub type ConvertedData<'py> = (
    Vec<String>,
    Option<Bound<'py, PyAny>>,
    Option<String>,
    Vec<String>,
    Option<Vec<Vec<String>>>,
);
