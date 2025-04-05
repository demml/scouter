use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use datafusion::dataframe::DataFrame;
use datafusion::dataframe::DataFrameWriteOptions;
use datafusion::datasource::MemTable;
use scouter_error::{ScouterError, StorageError};
use scouter_settings::ObjectStorageSettings;
use scouter_types::ToDriftRecords;
use scouter_types::{CustomMetricServerRecord, ServerRecords};
use std::sync::Arc;

#[async_trait]
pub trait ParquetFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError>
    where
        Self: Sized;

    async fn write_parquet(&self, rpath: &str, records: ServerRecords) -> Result<(), ScouterError>;
}

pub struct CustomMetricDataFrame {
    schema: Arc<Schema>,
    object_store: ObjectStore,
}
