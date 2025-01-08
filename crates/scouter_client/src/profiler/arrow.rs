use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
use num_traits::Float;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct ArrowDataConverter;

impl DataConverter for ArrowDataConverter {
    fn check_for_non_numeric(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let py = data.py();
        let mut string_features = Vec::new();
        let mut numeric_features = Vec::new();
        let features = data.getattr("column_names")?.extract::<Vec<String>>()?;
        let schema = data.getattr("schema")?;

        for feature in  features {
            let dtype =schema.call_method1("field", (&feature,))?.getattr("type")?;
            // assert dtype does not in [pa.int8(), pa.int16(), pa.int32(), pa.int64(), pa.float32(), pa.float64()]
            let pa_types = py.import("pyarrow")?.getattr("types")?;

            if pa_types.call_method1("is_integer", (&dtype,))?.extract::<bool>()? {
                numeric_features.push(feature);
            } else if pa_types.call_method1("is_floating", (&dtype,))?.extract::<bool>()? {
                numeric_features.push(feature);
            } else if pa_types.call_method1("is_decimal", (&dtype,))?.extract::<bool>()? {
                numeric_features.push(feature);
            } else {
                string_features.push(feature);
            }

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
        let py = data.py();
        let (numeric_features, string_features) = ArrowDataConverter::check_for_non_numeric(data)?;

        let numeric_array = if !&numeric_features.is_empty() {
            let array = numeric_features.iter().map(|feature| {
                let array = data.call_method1("column", (&feature,))?.call_method0("to_numpy")?;

                Ok(array)
            }).collect::<Result<Vec<Bound<'py, PyAny>>, ScouterError>>()?;

            let numpy = py.import("numpy")?;

            // call numpy.column_stack on array
            let array = numpy.call_method1("column_stack", (array,))?;
            let array = self.convert_array_type(&array)?;

            Some(array)
        } else {
            None
        };

        let string_array = if !&string_features.is_empty() {
            let array = string_features.iter().map(|feature| {
                let array = data.call_method1("column", (&feature,))?.call_method0("to_pylist")?.extract::<Vec<String>>()?;
                Ok(array)
            }).collect::<Result<Vec<Vec<String>>, ScouterError>>()?;
            Some(array)
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
