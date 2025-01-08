use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
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

    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), ScouterError> {
        if features.is_empty() {
            return Ok((None, None));
        }
        let dtype = Some(data.getattr("dtype")?.str()?.to_string());

        Ok((Some(data.clone()), dtype))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, ScouterError> {
        if features.is_empty() {
            return Ok(None);
        }

        Ok(Some(
            data.call_method1("astype", ("str",))?
                .call_method0("to_list")?
                .extract::<Vec<Vec<String>>>()?,
        ))
    }

    fn prepare_data<'py>(data: &Bound<'py, PyAny>) -> Result<ConvertedArray<'py>, ScouterError> {
        let (numeric_features, string_features) = NumpyDataConverter::check_for_non_numeric(data)?;

        let (numeric_array, dtype) =
            NumpyDataConverter::process_numeric_features(data, &numeric_features)?;
        let string_array = NumpyDataConverter::process_string_features(data, &string_features)?;

        Ok((
            numeric_features,
            numeric_array,
            dtype,
            string_features,
            string_array,
        ))
    }
}
