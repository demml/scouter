use crate::profiler::types::ConvertedArray;
use num_traits::Float;
use numpy::PyArray2;
use numpy::PyArrayMethods;
use numpy::PyReadonlyArray2;
use pyo3::{prelude::*, types::PyString};
use scouter_error::ScouterError;

pub trait DataConverter {
    fn check_for_non_numeric(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError>;

    fn convert_array_type<'py, F>(
        &self,
        data: &Bound<'py, PyAny>,
    ) -> Result<PyReadonlyArray2<'py, F>, ScouterError>
    where
        F: Float + numpy::Element,
    {
        let dtype = data
            .getattr("dtype")?
            .downcast::<PyString>()
            .map_err(|_| ScouterError::Error("Failed to downcast dtype".to_string()))?
            .to_string_lossy()
            .to_string();

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

    fn prepare_data<'py, F>(
        &self,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedArray<'py, F>, ScouterError>
    where
        F: Float + numpy::Element;
}
