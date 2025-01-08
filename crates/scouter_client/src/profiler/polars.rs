use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
use num_traits::Float;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct PolarsDataConverter;

impl DataConverter for PolarsDataConverter {
    fn check_for_non_numeric(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let mut numeric_features = Vec::new();
        let mut string_features = Vec::new();
        let columns = data.getattr("columns")?.extract::<Vec<String>>()?;
        let schema = data.getattr("schema")?;

        columns
            .iter()
            .map(|col| {
                let dtype = schema
                    .get_item(col)?
                    .getattr("dtype")?
                    .call_method0("is_numeric")?;

                if dtype.extract::<bool>()? {
                    numeric_features.push(col.clone());
                } else {
                    string_features.push(col.clone());
                }

                Ok(())
            })
            .collect::<Result<Vec<_>, ScouterError>>()?;

        Ok((numeric_features, string_features))
    }

    fn prepare_data<'py, F>(
        &self,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedArray<'py, F>, ScouterError>
    where
        F: Float + numpy::Element,
    {
        let (numeric_features, string_features) = PolarsDataConverter::check_for_non_numeric(data)?;

        let numeric_array = if !&numeric_features.is_empty() {
            // create slice of numeric columns
            let array = data.get_item(&numeric_features)?.call_method0("to_numpy")?;

            // downcast to PyArray2
            let array = self.convert_array_type(&array)?;
            Some(array)
        } else {
            None
        };

        let string_array = if !&string_features.is_empty() {
            Some(
                string_features
                    .iter()
                    .map(|feature| {
                        let array = data
                            .get_item(&feature)?
                            .call_method0("to_list")?
                            .extract::<Vec<String>>()?;
                        Ok(array)
                    })
                    .collect::<Result<Vec<Vec<String>>, ScouterError>>()?,
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
