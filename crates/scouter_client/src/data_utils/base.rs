use crate::data_utils::ConvertedData;
use num_traits::Float;
use numpy::PyArray2;
use numpy::PyArrayMethods;
use numpy::PyReadonlyArray2;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub fn convert_array_type<'py, F>(
    data: Bound<'py, PyAny>,
    dtype: &str,
) -> Result<PyReadonlyArray2<'py, F>, ScouterError>
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
        .map_err(|e| ScouterError::Error(e.to_string()))?;

    Ok(array.readonly())
}

pub trait DataConverter {
    fn categorize_features(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError>;

    #[allow(clippy::needless_lifetimes)]
    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), ScouterError>;

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, ScouterError>;

    fn prepare_data<'py>(data: &Bound<'py, PyAny>) -> Result<ConvertedData<'py>, ScouterError>;
}
