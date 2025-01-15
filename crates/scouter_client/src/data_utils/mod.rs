use pyo3::prelude::*;
use scouter_error::ScouterError;

pub mod arrow;
pub mod base;
pub mod numpy;
pub mod pandas;
pub mod polars;
pub mod types;

pub use arrow::*;
pub use base::*;
pub use numpy::*;
pub use pandas::*;
pub use polars::*;
use scouter_types::DataType;
pub use types::*;

pub enum DataConverterEnum {
    Arrow(ArrowDataConverter),
    Numpy(NumpyDataConverter),
    Pandas(PandasDataConverter),
    Polars(PolarsDataConverter),
}

impl DataConverterEnum {
    /// Convert the data to the appropriate format
    ///
    /// # Arguments
    ///
    /// * `data_type` - The type of data to convert
    /// * `data` - The data to convert
    ///
    /// # Returns
    ///
    /// The converted data
    pub fn convert_data<'py>(
        data_type: &DataType,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, ScouterError> {
        match data_type {
            DataType::Arrow => ArrowDataConverter::prepare_data(data),
            DataType::Numpy => NumpyDataConverter::prepare_data(data),
            DataType::Pandas => PandasDataConverter::prepare_data(data),
            DataType::Polars => PolarsDataConverter::prepare_data(data),
        }
    }
}
