use crate::profiler::base::DataConverter;
use crate::profiler::types::ConvertedArray;
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct PolarsDataConverter;

impl DataConverter for PolarsDataConverter {
    fn check_for_non_numeric(
        data: &Bound<'_, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let py = data.py();
        let cs = py.import("polars")?.getattr("selectors")?;

        let columns = data.getattr("columns")?.extract::<Vec<String>>()?;
        let numeric_features = data
            .call_method1("select", (&cs.call_method0("numeric")?,))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let string_features = columns
            .iter()
            .filter(|col| !numeric_features.contains(col))
            .cloned()
            .collect();

        Ok((numeric_features, string_features))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        features: &[String],
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), ScouterError> {
        if features.is_empty() {
            return Ok((None, None));
        }

        let array = data.get_item(features)?.call_method0("to_numpy")?;
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

        Ok(Some(
            features
                .iter()
                .map(|feature| {
                    let array = data
                        .get_item(feature)?
                        .call_method0("to_list")?
                        .extract::<Vec<String>>()?;
                    Ok(array)
                })
                .collect::<Result<Vec<Vec<String>>, ScouterError>>()?,
        ))
    }

    fn prepare_data<'py>(data: &Bound<'py, PyAny>) -> Result<ConvertedArray<'py>, ScouterError> {
        let (numeric_features, string_features) = PolarsDataConverter::check_for_non_numeric(data)?;

        let (numeric_array, dtype) =
            PolarsDataConverter::process_numeric_features(data, &numeric_features)?;
        let string_array = PolarsDataConverter::process_string_features(data, &string_features)?;

        Ok((
            numeric_features,
            numeric_array,
            dtype,
            string_features,
            string_array,
        ))
    }
}
