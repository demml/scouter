// This module contains dataframe operations for LLM drift records (input, response, context, prompt).
use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Int32Array, StringArray, TimestampNanosecondArray};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;

use scouter_types::{LLMEventRecord, ServerRecords, StorageType, ToDriftRecords};
use std::sync::Arc;

pub struct LLMEventDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for LLMEventDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        LLMEventDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        let records = records.to_llm_event_records()?;
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
        _bin: &f64,
        _start_time: &DateTime<Utc>,
        _end_time: &DateTime<Utc>,
        _space: &str,
        _name: &str,
        _version: &str,
    ) -> String {
        "None".to_string()
    }

    fn table_name(&self) -> String {
        BinnedTableName::LLMEvent.to_string()
    }
}

impl LLMEventDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("uid", DataType::Utf8, false),
            Field::new("space", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("version", DataType::Utf8, false),
            Field::new("context", DataType::Utf8, false),
            Field::new("prompt", DataType::Utf8, true),
            Field::new("score", DataType::Utf8, true),
            Field::new(
                "updated_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new("status", DataType::Utf8, false),
            Field::new(
                "processing_started_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new(
                "processing_ended_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
            Field::new("processing_duration", DataType::Int32, true),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(LLMDriftDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(
        &self,
        records: Vec<LLMDriftServerRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        let id_array = arrow_array::Int64Array::from_iter_values(records.iter().map(|r| r.id));
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );
        let uid_array = StringArray::from_iter_values(records.iter().map(|r| r.uid.as_str()));
        let space_array = StringArray::from_iter_values(records.iter().map(|r| r.space.as_str()));
        let name_array = StringArray::from_iter_values(records.iter().map(|r| r.name.as_str()));
        let version_array =
            StringArray::from_iter_values(records.iter().map(|r| r.version.as_str()));

        let score_array = StringArray::from_iter_values(
            records
                .iter()
                .map(|r| serde_json::to_string(&r.score).unwrap_or_else(|_| "{}".to_string())),
        );
        let context_array = StringArray::from_iter_values(
            records
                .iter()
                .map(|r| serde_json::to_string(&r.context).unwrap_or_else(|_| "{}".to_string())),
        );

        let prompt_array = StringArray::from_iter(records.iter().map(|r| {
            r.prompt
                .as_ref()
                .map(|p| serde_json::to_string(p).unwrap_or_else(|_| "{}".to_string()))
        }));
        let updated_at_array = TimestampNanosecondArray::from_iter(
            records
                .iter()
                .map(|r| r.updated_at.and_then(|dt| dt.timestamp_nanos_opt())),
        );
        let status_array =
            StringArray::from_iter_values(records.iter().map(|r| r.status.to_string()));

        let processing_started_at_array =
            TimestampNanosecondArray::from_iter(records.iter().map(|r| {
                r.processing_started_at
                    .and_then(|dt| dt.timestamp_nanos_opt())
            }));

        let processing_ended_at_array =
            TimestampNanosecondArray::from_iter(records.iter().map(|r| {
                r.processing_ended_at
                    .and_then(|dt| dt.timestamp_nanos_opt())
            }));

        // Calculate processing duration in seconds
        let processing_duration_array =
            Int32Array::from_iter(records.iter().map(|r| r.processing_duration));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(id_array),
                Arc::new(created_at_array),
                Arc::new(uid_array),
                Arc::new(space_array),
                Arc::new(name_array),
                Arc::new(version_array),
                Arc::new(context_array),
                Arc::new(prompt_array),
                Arc::new(score_array),
                Arc::new(updated_at_array),
                Arc::new(status_array),
                Arc::new(processing_started_at_array),
                Arc::new(processing_ended_at_array),
                Arc::new(processing_duration_array),
            ],
        )?;

        Ok(batch)
    }
}
