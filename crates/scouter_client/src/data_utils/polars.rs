use crate::data_utils::types::DataTypes;
use crate::data_utils::{ConvertedData, DataConverter};
use crate::error::DataError;
use pyo3::prelude::*;
pub struct PolarsDataConverter;

impl DataConverter for PolarsDataConverter {
    fn categorize_features<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<DataTypes, DataError> {
        let cs = py.import("polars")?.getattr("selectors")?;

        let columns = data.getattr("columns")?.extract::<Vec<String>>()?;

        let integer_features = data
            .call_method1("select", (&cs.call_method0("integer")?,))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let float_features = data
            .call_method1("select", (&cs.call_method0("float")?,))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let string_features = columns
            .iter()
            .filter(|col| !float_features.contains(col) && !integer_features.contains(col))
            .cloned()
            .collect();

        Ok(DataTypes::new(
            integer_features,
            float_features,
            string_features,
        ))
    }

    #[allow(clippy::needless_lifetimes)]
    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        data_types: &DataTypes,
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), DataError> {
        if data_types.numeric_features.is_empty() {
            return Ok((None, None));
        }

        // If mixed types, we cast to Float64 to ensure consistency
        let array = if data_types.has_mixed_types() {
            let py = data.py();
            let float64 = py.import("polars")?.getattr("Float64")?;

            data.get_item(&data_types.numeric_features)?
                .call_method1("cast", (float64,))?
                .call_method0("to_numpy")?
        } else {
            data.get_item(&data_types.numeric_features)?
                .call_method0("to_numpy")?
        };

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

        let py = data.py();
        let polars = py.import("polars")?;
        let pl_string = polars.getattr("String")?;

        Ok(Some(
            features
                .iter()
                .map(|feature| {
                    let array = data
                        .get_item(feature)?
                        .call_method1("cast", (pl_string.clone(),))?
                        .call_method0("to_list")?
                        .extract::<Vec<String>>()?;
                    Ok(array)
                })
                .collect::<Result<Vec<Vec<String>>, DataError>>()?,
        ))
    }

    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError> {
        let data_types = PolarsDataConverter::categorize_features(py, data)?;

        let (numeric_array, dtype) =
            PolarsDataConverter::process_numeric_features(data, &data_types)?;
        let string_array =
            PolarsDataConverter::process_string_features(data, &data_types.string_features)?;

        Ok((
            data_types.numeric_features,
            numeric_array,
            dtype,
            data_types.string_features,
            string_array,
        ))
    }
}
