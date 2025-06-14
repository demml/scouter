use crate::data_utils::types::DataTypes;
use crate::data_utils::{ConvertedData, DataConverter};
use crate::error::DataError;
use pyo3::prelude::*;
use tracing::{debug, instrument};

pub struct PandasDataConverter;

impl DataConverter for PandasDataConverter {
    #[instrument(skip_all)]
    fn categorize_features<'py>(
        _py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<DataTypes, DataError> {
        let column_name_dtype = data
            .getattr("columns")?
            .getattr("dtype")?
            .str()?
            .to_string();

        if !column_name_dtype.contains("object") {
            return Err(DataError::ColumnNamesMustBeStrings);
        }

        let all_columns = data.getattr("columns")?.extract::<Vec<String>>()?;

        // get integer and float columns
        let integer_columns = data
            .call_method1("select_dtypes", ("integer",))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let float_columns = data
            .call_method1("select_dtypes", ("float",))?
            .getattr("columns")?
            .extract::<Vec<String>>()?;

        let non_numeric_columns: Vec<String> = all_columns
            .iter()
            .filter(|col| !float_columns.contains(col) && !integer_columns.contains(col))
            .cloned()
            .collect();

        debug!("Non-numeric columns: {:?}", non_numeric_columns);

        // Introducing specific numeric types because we may want to handle them differently at a later point
        Ok(DataTypes::new(
            integer_columns,
            float_columns,
            non_numeric_columns,
        ))
    }

    fn process_numeric_features<'py>(
        data: &Bound<'py, PyAny>,
        data_types: &DataTypes,
    ) -> Result<(Option<Bound<'py, PyAny>>, Option<String>), DataError> {
        if data_types.numeric_features.is_empty() {
            return Ok((None, None));
        }

        // if mixed type is true, it assumes we are at least dealing with float and integer types. we will need to convert all to float64
        let array = if data_types.has_mixed_types() {
            data.get_item(&data_types.numeric_features)?
                .call_method1("astype", ("float64",))?
                .call_method0("to_numpy")?
        } else {
            data.get_item(&data_types.numeric_features)?
                .call_method0("to_numpy")?
        };

        let dtype = Some(array.getattr("dtype")?.str()?.to_string());

        //

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

        let string_cols = data
            .get_item(features)?
            .call_method1("astype", ("str",))?
            .getattr("values")?
            .getattr("T")?
            .call_method0("tolist")?;
        let string_array = string_cols.extract::<Vec<Vec<String>>>()?;

        Ok(Some(string_array))
    }

    #[instrument(skip_all)]
    fn prepare_data<'py>(
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError> {
        let data_types = PandasDataConverter::categorize_features(py, data)?;

        let (numeric_array, dtype) =
            PandasDataConverter::process_numeric_features(data, &data_types)?;

        let string_array =
            PandasDataConverter::process_string_features(data, &data_types.string_features)?;

        Ok((
            data_types.numeric_features,
            numeric_array,
            dtype,
            data_types.string_features,
            string_array,
        ))
    }
}
