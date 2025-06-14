use crate::data_utils::{ConvertedData, DataTypes};

use crate::error::DataError;
use num_traits::Float;
use numpy::PyArray2;
use numpy::PyArrayMethods;
use numpy::PyReadonlyArray2;
use pyo3::prelude::*;

pub fn convert_array_type<'py, F>(
    data: Bound<'py, PyAny>,
    dtype: &str,
) -> Result<PyReadonlyArray2<'py, F>, DataError>
where
    F: Float + numpy::Element,
{
    let array = if dtype.contains("int") {
        data.call_method1("astype", ("float32",))?
    } else {
        data.clone()
    };

    let array = array
        .downcast_into::<PyArray2<F>>()
        .map_err(|e| DataError::DowncastError(e.to_string()))?;

    Ok(array.readonly())
}

pub trait DataConverter {
    fn categorize_features<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<DataTypes, DataError>;

    #[allow(clippy::needless_lifetimes)]
    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        data_type: &DataTypes,
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), DataError>;

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, DataError>;

    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError>;
}
