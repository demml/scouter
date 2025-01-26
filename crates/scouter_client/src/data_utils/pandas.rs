use crate::data_utils::{ConvertedData, DataConverter};
use pyo3::prelude::*;
use scouter_error::ScouterError;

pub struct PandasDataConverter;

impl DataConverter for PandasDataConverter {
    fn categorize_features<'py>(
        _py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<(Vec<String>, Vec<String>), ScouterError> {
        let column_name_dtype = data
            .getattr("columns")?
            .getattr("dtype")?
            .str()?
            .to_string();

        if !column_name_dtype.contains("object") {
            return Err(ScouterError::Error(
                "Column names must be string type".to_string(),
            ));
        }

        let all_columns = data.getattr("columns")?.extract::<Vec<String>>()?;

        // Check for non-numeric columns
        let numeric_columns = data
            .call_method1("select_dtypes", ("number",))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let non_numeric_columns: Vec<String> = all_columns
            .iter()
            .filter(|col| !numeric_columns.contains(col))
            .cloned()
            .collect();

        Ok((numeric_columns, non_numeric_columns))
    }

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

        let string_cols = data
            .get_item(features)?
            .call_method1("astype", ("str",))?
            .getattr("values")?
            .getattr("T")?
            .call_method0("tolist")?;
        let string_array = string_cols.extract::<Vec<Vec<String>>>()?;

        Ok(Some(string_array))
    }

    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, ScouterError> {
        let (numeric_features, string_features) =
            PandasDataConverter::categorize_features(py, data)?;

        let (numeric_array, dtype) =
            PandasDataConverter::process_numeric_features(data, &numeric_features)?;
        let string_array = PandasDataConverter::process_string_features(data, &string_features)?;

        Ok((
            numeric_features,
            numeric_array,
            dtype,
            string_features,
            string_array,
        ))
    }
}
