use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
use num_traits::Float;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct NumpyDataConverter;

impl DataConverter for NumpyDataConverter {
    fn check_for_non_numeric(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let py = data.py();
        let numpy = PyModule::import(py, "numpy")?.getattr("ndarray")?;

        if !data.is_instance(&numpy)? {
            return Err(ScouterError::Error("Data is not a numpy array".to_string()));
        }

        let mut string_features = Vec::new();
        let mut numeric_features = Vec::new();
        let shape = data.getattr("shape")?.extract::<Vec<usize>>()?;
        let dtypes = data.getattr("dtype")?;

        if dtypes.getattr("kind")?.extract::<String>()? == "u" {
            // create vec from shape[1]
            string_features = (0..shape[1])
                .map(|i| format!("feature_{}", i))
                .collect::<Vec<String>>();
        } else {
            numeric_features = (0..shape[1])
                .map(|i| format!("feature_{}", i))
                .collect::<Vec<String>>();
        }

        Ok((numeric_features, string_features))
    }

    fn prepare_data<'py, F>(
        &self,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedArray<'py, F>, ScouterError>
    where
        F: Float + numpy::Element,
    {
        let (numeric_features, string_features) = NumpyDataConverter::check_for_non_numeric(data)?;

        let numeric_array = if !&numeric_features.is_empty() {
            let array = self.convert_array_type(&data)?;
            Some(array)
        } else {
            None
        };

        let string_array = if !&string_features.is_empty() {
            Some(
                data.call_method1("astype", ("str",))?
                    .call_method0("to_list")?
                    .extract::<Vec<Vec<String>>>()?,
            )
        } else {
            None
        };

        Ok((
            numeric_features,
            numeric_array,
            string_features,
            string_array,
        ))
    }
}
