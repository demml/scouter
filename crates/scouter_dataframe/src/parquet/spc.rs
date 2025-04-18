use super::types::BinnedTableName;
use crate::parquet::traits::ParquetFrame;
use crate::sql::helper::get_binned_spc_drift_records_query;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_error::ScouterError;
use scouter_settings::ObjectStorageSettings;

use scouter_types::{ServerRecords, SpcServerRecord};
use scouter_types::{StorageType, ToDriftRecords};
use std::sync::Arc;
pub struct SpcDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for SpcDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError> {
        SpcDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, ScouterError> {
        let records = records.to_spc_drift_records()?;
        let batch = self.build_batch(records)?;

        let ctx = self.object_store.get_session()?;

        let df = ctx
            .read_batches(vec![batch])
            .map_err(|e| ScouterError::Error(format!("Failed to read batches: {}", e)))?;

        Ok(df)
    }

    fn storage_root(&self) -> String {
        self.object_store.storage_settings.canonicalized_path()
    }

    fn storage_type(&self) -> StorageType {
        self.object_store.storage_settings.storage_type.clone()
    }

    fn get_session_context(&self) -> Result<SessionContext, ScouterError> {
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
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError> {
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

    pub fn build_batch(&self, records: Vec<SpcServerRecord>) -> Result<RecordBatch, ScouterError> {
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

        RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at),
                Arc::new(space),
                Arc::new(name),
                Arc::new(version),
                Arc::new(feature),
                Arc::new(value),
            ],
        )
        .map_err(|e| ScouterError::Error(format!("Failed to create record batch: {}", e)))
    }
}
