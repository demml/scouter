use crate::profiler::types::ConvertedArray;
use numpy::{PyArray2, PyArrayMethods};
use pyo3::prelude::*;
use scouter_error::ScouterError;

fn check_for_non_numeric(
    data: &Bound<'_, PyAny>,
) -> Result<(Vec<String>, Vec<String>), ScouterError> {
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

pub fn prepare_pandas_data<'py>(
    data: &Bound<'py, PyAny>,
) -> Result<ConvertedArray<'py>, ScouterError> {
    let (numeric_columns, non_numeric_columns) = check_for_non_numeric(data)?;

    let numeric_array = if !&numeric_columns.is_empty() {
        // create slice of numeric columns
        let array = data
            .get_item(&numeric_columns)?
            .call_method0("to_numpy")?
            .call_method1("astype", ("float64",))?;

        // downcast to PyArray2
        let numeric_array = array
            .downcast_into::<PyArray2<f64>>()
            .map_err(|e| ScouterError::Error(e.to_string()))?;
        let owned = numeric_array.readonly();

        Some(owned)
    } else {
        None
    };

    let string_array = if !&non_numeric_columns.is_empty() {
        let string_cols = data
            .get_item(&non_numeric_columns)?
            .call_method1("astype", ("str",))?
            .getattr("values")?
            .getattr("T")?
            .call_method0("tolist")?;
        let string_array = string_cols.extract::<Vec<Vec<String>>>()?;

        Some(string_array)
    } else {
        None
    };

    Ok((
        numeric_columns,
        numeric_array,
        non_numeric_columns,
        string_array,
    ))
}
