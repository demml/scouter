use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct ArrowDataConverter;

impl DataConverter for ArrowDataConverter {
    #[allow(clippy::if_same_then_else)]
    fn categorize_features(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let py = data.py();
        let mut string_features = Vec::new();
        let mut numeric_features = Vec::new();
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
                numeric_features.push(feature);
            } else if pa_types
                .call_method1("is_floating", (&dtype,))?
                .extract::<bool>()?
            {
                numeric_features.push(feature);
            } else if pa_types
                .call_method1("is_decimal", (&dtype,))?
                .extract::<bool>()?
            {
                numeric_features.push(feature);
            } else {
                string_features.push(feature);
            }
        }

        Ok((numeric_features, string_features))
    }

    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), ScouterError> {
        let py = data.py();
        if features.is_empty() {
            return Ok((None, None));
        }

        let array = features
            .iter()
            .map(|feature| {
                let array = data
                    .call_method1("column", (&feature,))?
                    .call_method0("to_numpy")?;

                Ok(array)
            })
            .collect::<Result<Vec<Bound<'py, PyAny>>, ScouterError>>()?;

        let numpy = py.import("numpy")?;

        // call numpy.column_stack on array
        let array = numpy.call_method1("column_stack", (array,))?;
        //let array = ArrowDataConverter::convert_array_type(&array)?;
        let dtype = Some(array.getattr("dtype")?.str()?.to_string());

        Ok((Some(array), dtype))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_string_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<Option<Vec<Vec<String>>>, ScouterError> {
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
            .collect::<Result<Vec<Vec<String>>, ScouterError>>()?;
        Ok(Some(array))
    }

    fn prepare_data<'py>(data: &Bound<'py, PyAny>) -> Result<ConvertedArray<'py>, ScouterError> {
        let (numeric_features, string_features) = ArrowDataConverter::categorize_features(data)?;

        let (numeric_array, dtype) =
            ArrowDataConverter::process_numeric_features(data, &numeric_features)?;
        let string_array = ArrowDataConverter::process_string_features(data, &string_features)?;

        Ok((
            numeric_features,
            numeric_array,
            dtype,
            string_features,
            string_array,
        ))
    }
}
