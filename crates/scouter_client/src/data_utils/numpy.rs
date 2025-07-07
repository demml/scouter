use crate::data_utils::{ConvertedData, DataConverter, DataTypes};
use crate::error::DataError;
use pyo3::prelude::*;

pub struct NumpyDataConverter;

impl DataConverter for NumpyDataConverter {
    fn categorize_features<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<DataTypes, DataError> {
        let numpy = py.import("numpy")?.getattr("ndarray")?;

        if !data.is_instance(&numpy)? {
            return Err(DataError::NotNumpyArrayError);
        }

        let mut string_features = Vec::new();
        let mut float_features = Vec::new();

        let shape = data.getattr("shape")?.extract::<Vec<usize>>()?;
        let dtypes = data.getattr("dtype")?;

        if dtypes.getattr("kind")?.extract::<String>()? == "u" {
            // create vec from shape[1]
            string_features = (0..shape[1])
                .map(|i| format!("feature_{i}"))
                .collect::<Vec<String>>();
        } else {
            float_features = (0..shape[1])
                .map(|i| format!("feature_{i}"))
                .collect::<Vec<String>>();
        }

        Ok(DataTypes::new(
            Vec::new(), // No integer features in numpy
            float_features,
            string_features,
        ))
    }

    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        data_types: &DataTypes,
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), DataError> {
        if data_types.numeric_features.is_empty() {
            return Ok((None, None));
        }
        let dtype = Some(data.getattr("dtype")?.str()?.to_string());

        Ok((Some(data.clone()), dtype))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, DataError> {
        if features.is_empty() {
            return Ok(None);
        }

        Ok(Some(
            data.call_method1("astype", ("str",))?
                .call_method0("to_list")?
                .extract::<Vec<Vec<String>>>()?,
        ))
    }

    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError> {
        let data_types = NumpyDataConverter::categorize_features(py, data)?;

        let (numeric_array, dtype) =
            NumpyDataConverter::process_numeric_features(data, &data_types)?;
        let string_array =
            NumpyDataConverter::process_string_features(data, &data_types.string_features)?;

        Ok((
            data_types.numeric_features,
            numeric_array,
            dtype,
            data_types.string_features,
            string_array,
        ))
    }
}
