use crate::data_utils::{ConvertedData, DataConverter, DataTypes};
use crate::error::DataError;
use pyo3::prelude::*;

pub struct ArrowDataConverter;

impl DataConverter for ArrowDataConverter {
    #[allow(clippy::if_same_then_else)]
    fn categorize_features<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<DataTypes, DataError> {
        let mut string_features = Vec::new();
        let mut integer_features = Vec::new();
        let mut float_features = Vec::new();
        let features = data.getattr("column_names")?.extract::<Vec<String>>()?;
        let schema = data.getattr("schema")?;

        for feature in features {
            let dtype = schema.call_method1("field", (&feature,))?.getattr("type")?;
            // assert dtype does not in [pa.int8(), pa.int16(), pa.int32(), pa.int64(), pa.float32(), pa.float64()]
            let pa_types = py.import("pyarrow")?.getattr("types")?;

            if pa_types
                .call_method1("is_integer", (&dtype,))?
                .extract::<bool>()?
            {
                integer_features.push(feature);
            } else if pa_types
                .call_method1("is_floating", (&dtype,))?
                .extract::<bool>()?
            {
                float_features.push(feature);
            } else if pa_types
                .call_method1("is_decimal", (&dtype,))?
                .extract::<bool>()?
            {
                float_features.push(feature);
            } else {
                string_features.push(feature);
            }
        }

        Ok(DataTypes::new(
            integer_features,
            float_features,
            string_features,
        ))
    }

    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        data_types: &DataTypes,
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), DataError> {
        let py = data.py();
        if data_types.numeric_features.is_empty() {
            return Ok((None, None));
        }

        let is_mixed_type = data_types.has_mixed_types();

        let array = data_types
            .numeric_features
            .iter()
            .map(|feature| {
                let array = data
                    .call_method1("column", (&feature,))?
                    .call_method0("to_numpy")?;

                // Convert all to f64
                if is_mixed_type {
                    Ok(array.call_method1("astype", ("float64",))?)
                } else {
                    Ok(array)
                }
            })
            .collect::<Result<Vec<Bound<'py, PyAny>>, DataError>>()?;

        let numpy = py.import("numpy")?;

        // call numpy.column_stack on array
        let array = numpy.call_method1("column_stack", (array,))?;
        let dtype = Some(array.getattr("dtype")?.str()?.to_string());

        Ok((Some(array), dtype))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, DataError> {
        if features.is_empty() {
            return Ok(None);
        }

        let array = features
            .iter()
            .map(|feature| {
                let array = data
                    .call_method1("column", (&feature,))?
                    .call_method0("to_pylist")?
                    .extract::<Vec<String>>()?;
                Ok(array)
            })
            .collect::<Result<Vec<Vec<String>>, DataError>>()?;
        Ok(Some(array))
    }

    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError> {
        let data_types = ArrowDataConverter::categorize_features(py, data)?;

        let (numeric_array, dtype) =
            ArrowDataConverter::process_numeric_features(data, &data_types)?;
        let string_array =
            ArrowDataConverter::process_string_features(data, &data_types.string_features)?;

        Ok((
            data_types.numeric_features,
            numeric_array,
            dtype,
            data_types.string_features,
            string_array,
        ))
    }
}
