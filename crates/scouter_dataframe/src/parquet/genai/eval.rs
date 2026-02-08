// This module contains dataframe operations for GenAI drift records (input, response, context, prompt).
use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{
    DictionaryArray, FixedSizeBinaryArray, Int32Array, Int64Array, StringArray,
    TimestampNanosecondArray, UInt32Array, UInt8Array,
};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::ToDriftRecords;
use scouter_types::{BoxedGenAIEvalRecord, ServerRecords, StorageType};
use std::sync::Arc;

pub struct GenAIEvalDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for GenAIEvalDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        GenAIEvalDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        let records = records.to_genai_eval_records()?;
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
        _entity_id: &i32,
    ) -> String {
        "None".to_string()
    }

    fn table_name(&self) -> String {
        BinnedTableName::GenAIEval.to_string()
    }
}

impl GenAIEvalDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            // Primary keys and identifiers
            Field::new("id", DataType::Int64, false),
            Field::new("uid", DataType::Utf8, false),
            Field::new("entity_id", DataType::Int32, false),
            Field::new("entity_uid", DataType::Utf8, false),
            Field::new(
                "entity_type",
                DataType::Dictionary(Box::new(DataType::UInt8), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new(
                "record_id",
                DataType::Dictionary(Box::new(DataType::UInt32), Box::new(DataType::Utf8)),
                true,
            ),
            Field::new(
                "session_id",
                DataType::Dictionary(Box::new(DataType::UInt32), Box::new(DataType::Utf8)),
                true,
            ),
            Field::new(
                "status",
                DataType::Dictionary(Box::new(DataType::UInt8), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "updated_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                true,
            ),
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
            Field::new("retry_count", DataType::Int32, false),
            Field::new("context", DataType::Utf8, false), // Consider LargeUtf8 if contexts are very large
            Field::new("trace_id", DataType::FixedSizeBinary(16), true),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(GenAIEvalDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(
        &self,
        records: Vec<BoxedGenAIEvalRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        // Build ID and UID arrays
        let id_array = Int64Array::from_iter_values(records.iter().map(|r| r.record.id));
        let uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.record.uid.as_str()));

        let entity_id_array =
            Int32Array::from_iter_values(records.iter().map(|r| r.record.entity_id));

        let entity_uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.record.entity_uid.as_str()));

        let entity_type_values =
            StringArray::from_iter_values(records.iter().map(|r| r.record.entity_type.to_string()));
        let entity_type_keys = UInt8Array::from_iter_values(0..records.len() as u8);
        let entity_type_array =
            DictionaryArray::new(entity_type_keys, Arc::new(entity_type_values));

        let record_id_values =
            StringArray::from_iter_values(records.iter().map(|r| r.record.record_id.as_str()));
        let record_id_keys = UInt32Array::from_iter_values(0..records.len() as u32);
        let record_id_array = DictionaryArray::new(record_id_keys, Arc::new(record_id_values));

        let session_id_values =
            StringArray::from_iter_values(records.iter().map(|r| r.record.session_id.as_str()));
        let session_id_keys = UInt32Array::from_iter_values(0..records.len() as u32);
        let session_id_array = DictionaryArray::new(session_id_keys, Arc::new(session_id_values));

        let status_values =
            StringArray::from_iter_values(records.iter().map(|r| r.record.status.to_string()));
        let status_keys = UInt8Array::from_iter_values(0..records.len() as u8);
        let status_array = DictionaryArray::new(status_keys, Arc::new(status_values));

        let created_at_array =
            TimestampNanosecondArray::from_iter_values(records.iter().map(|r| {
                r.record
                    .created_at
                    .timestamp_nanos_opt()
                    .unwrap_or_default()
            }));
        let updated_at_array = TimestampNanosecondArray::from_iter(
            records
                .iter()
                .map(|r| r.record.updated_at.and_then(|dt| dt.timestamp_nanos_opt())),
        );
        let processing_started_at_array =
            TimestampNanosecondArray::from_iter(records.iter().map(|r| {
                r.record
                    .processing_started_at
                    .and_then(|dt| dt.timestamp_nanos_opt())
            }));
        let processing_ended_at_array =
            TimestampNanosecondArray::from_iter(records.iter().map(|r| {
                r.record
                    .processing_ended_at
                    .and_then(|dt| dt.timestamp_nanos_opt())
            }));

        let processing_duration_array =
            Int32Array::from_iter(records.iter().map(|r| r.record.processing_duration));
        let retry_count_array =
            Int32Array::from_iter_values(records.iter().map(|r| r.record.retry_count));

        let context_array = StringArray::from_iter_values(records.iter().map(|r| {
            serde_json::to_string(&r.record.context).unwrap_or_else(|_| "{}".to_string())
        }));

        let trace_id_array = FixedSizeBinaryArray::try_from_sparse_iter_with_size(
            records.iter().map(|r| {
                r.record
                    .trace_id
                    .as_ref()
                    .map(|tid| tid.as_bytes().to_vec())
            }),
            16,
        )?;

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(id_array),
                Arc::new(uid_array),
                Arc::new(entity_id_array),
                Arc::new(entity_uid_array),
                Arc::new(entity_type_array),
                Arc::new(record_id_array),
                Arc::new(session_id_array),
                Arc::new(status_array),
                Arc::new(created_at_array),
                Arc::new(updated_at_array),
                Arc::new(processing_started_at_array),
                Arc::new(processing_ended_at_array),
                Arc::new(processing_duration_array),
                Arc::new(retry_count_array),
                Arc::new(context_array),
                Arc::new(trace_id_array),
            ],
        )?;

        Ok(batch)
    }
}
