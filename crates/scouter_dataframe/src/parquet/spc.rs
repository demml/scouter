use super::types::BinnedTableName;
use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::sql::helper::get_binned_spc_drift_records_query;
use crate::storage::ObjectStore;
use arrow::array::AsArray;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::spc::{SpcDriftFeature, SpcDriftFeatures};
use scouter_types::{ServerRecords, SpcServerRecord};
use scouter_types::{StorageType, ToDriftRecords};
use std::collections::BTreeMap;
use std::sync::Arc;

pub struct SpcDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for SpcDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        SpcDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        let records = records.to_spc_drift_records()?;
        let batch = self.build_batch(records)?;

        let ctx = self.object_store.get_session()?;

        let df = ctx.read_batches(vec![batch])?;

        Ok(df)
    }

    fn storage_root(&self) -> String {
        self.object_store.storage_settings.canonicalized_path()
    }

    fn storage_type(&self) -> StorageType {
        self.object_store.storage_settings.storage_type.clone()
    }

    fn get_session_context(&self) -> Result<SessionContext, DataFrameError> {
        Ok(self.object_store.get_session()?)
    }

    fn get_binned_sql(
        &self,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> String {
        get_binned_spc_drift_records_query(bin, start_time, end_time, space, name, version)
    }

    fn table_name(&self) -> String {
        BinnedTableName::Spc.to_string()
    }
}
impl SpcDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("space", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("version", DataType::Utf8, false),
            Field::new("feature", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(SpcDataFrame {
            schema,
            object_store,
        })
    }

    pub fn build_batch(
        &self,
        records: Vec<SpcServerRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        let created_at = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );
        let space = StringArray::from_iter_values(records.iter().map(|r| r.space.as_str()));
        let name = StringArray::from_iter_values(records.iter().map(|r| r.name.as_str()));
        let version = StringArray::from_iter_values(records.iter().map(|r| r.version.as_str()));
        let feature = StringArray::from_iter_values(records.iter().map(|r| r.feature.as_str()));
        let value = Float64Array::from_iter_values(records.iter().map(|r| r.value));

        Ok(RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at),
                Arc::new(space),
                Arc::new(name),
                Arc::new(version),
                Arc::new(feature),
                Arc::new(value),
            ],
        )?)
    }
}

/// Helper function to process a record batch to feature and SpcDriftFeature
///
/// # Arguments
/// * `batch` - The record batch to process
/// * `features` - The features to populate
///
/// # Returns
/// * `Result<(), DataFrameError>` - The result of the processing
fn process_spc_record_batch(
    batch: &RecordBatch,
    features: &mut BTreeMap<String, SpcDriftFeature>,
) -> Result<(), DataFrameError> {
    // Feature is the first column and is stringarray
    let feature_array = batch
        .column_by_name("feature")
        .ok_or_else(|| DataFrameError::MissingFieldError("feature"))?
        .as_string_view_opt()
        .ok_or_else(|| DataFrameError::DowncastError("feature"))?;

    // The created_at and values columns are lists<i32>
    let created_at_list = batch
        .column_by_name("created_at")
        .ok_or_else(|| DataFrameError::MissingFieldError("created_at"))?
        .as_list_opt::<i32>()
        .ok_or_else(|| DataFrameError::DowncastError("created_at"))?;

    let values_list = batch
        .column_by_name("values")
        .ok_or_else(|| DataFrameError::MissingFieldError("values"))?
        .as_list_opt::<i32>()
        .ok_or_else(|| DataFrameError::DowncastError("values"))?;

    for row in 0..batch.num_rows() {
        let feature_name = feature_array.value(row).to_string();

        // Convert timestamps to DateTime<Utc>
        let created_at = created_at_list
            .value(row)
            .as_primitive::<arrow::datatypes::TimestampNanosecondType>()
            .iter()
            .filter_map(|ts| ts.map(|t| Utc.timestamp_nanos(t)))
            .collect::<Vec<_>>();

        // Convert values to Vec<f64>
        let values = values_list
            .value(row)
            .as_primitive::<arrow::datatypes::Float64Type>()
            .iter()
            .flatten()
            .collect::<Vec<_>>();

        features.insert(feature_name, SpcDriftFeature { created_at, values });
    }

    Ok(())
}

/// Convert a DataFrame to SpcDriftFeatures
///
/// # Arguments
/// * `df` - The DataFrame to convert
///
/// # Returns
/// * `SpcDriftFeatures` - The converted SpcDriftFeatures
pub async fn dataframe_to_spc_drift_features(
    df: DataFrame,
) -> Result<SpcDriftFeatures, DataFrameError> {
    let batches = df.collect().await?;

    let mut features = BTreeMap::new();

    for batch in batches {
        process_spc_record_batch(&batch, &mut features)?;
    }

    Ok(SpcDriftFeatures { features })
}
