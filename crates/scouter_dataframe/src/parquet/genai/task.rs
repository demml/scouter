use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::sql::helper::get_binned_genai_task_values_query;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{
    BooleanArray, DictionaryArray, Float64Array, Int32Array, StringArray, TimestampNanosecondArray,
    UInt32Array, UInt8Array,
};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{GenAIEvalTaskResult, ServerRecords, StorageType, ToDriftRecords};
use std::sync::Arc;

pub struct GenAITaskDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for GenAITaskDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        GenAITaskDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        let records = records.to_genai_task_records()?;
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
        entity_id: &i32,
    ) -> String {
        get_binned_genai_task_values_query(bin, start_time, end_time, entity_id)
    }

    fn table_name(&self) -> String {
        BinnedTableName::GenAITask.to_string()
    }
}

impl GenAITaskDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("entity_id", DataType::Int32, false),
            Field::new("entity_uid", DataType::Utf8, false),
            Field::new("record_uid", DataType::Utf8, false),
            Field::new(
                "task_id",
                DataType::Dictionary(Box::new(DataType::UInt32), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new(
                "task_type",
                DataType::Dictionary(Box::new(DataType::UInt8), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new(
                "operator",
                DataType::Dictionary(Box::new(DataType::UInt8), Box::new(DataType::Utf8)),
                false,
            ),
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "start_time",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "end_time",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("stage", DataType::Int32, false),
            Field::new("passed", DataType::Boolean, false),
            Field::new("condition", DataType::Boolean, false),
            Field::new("value", DataType::Float64, false),
            Field::new("assertion", DataType::Utf8, true),
            Field::new("expected", DataType::Utf8, false),
            Field::new("actual", DataType::Utf8, false),
            Field::new("message", DataType::Utf8, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(GenAITaskDataFrame {
            schema,
            object_store,
        })
    }

    /// Builds an Arrow RecordBatch from GenAIEvalTaskResults
    /// # Arguments
    /// * `records` - A vector of references to GenAIEvalTaskResults
    /// # Returns
    /// * A RecordBatch containing the data from the records
    /// # Errors
    /// * DataFrameError if there is an issue creating the RecordBatch
    fn build_batch(
        &self,
        records: Vec<GenAIEvalTaskResult>,
    ) -> Result<RecordBatch, DataFrameError> {
        // Entity and record identifiers
        let entity_id_array = Int32Array::from_iter_values(records.iter().map(|r| r.entity_id));
        let entity_uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.entity_uid.as_str()));
        let record_uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.record_uid.as_str()));

        let task_id_values =
            StringArray::from_iter_values(records.iter().map(|r| r.task_id.as_str()));
        let task_id_keys = UInt32Array::from_iter_values(0..records.len() as u32);
        let task_id_array = DictionaryArray::new(task_id_keys, Arc::new(task_id_values));

        let task_type_values =
            StringArray::from_iter_values(records.iter().map(|r| r.task_type.as_str()));
        let task_type_keys = UInt8Array::from_iter_values(0..records.len() as u8);
        let task_type_array = DictionaryArray::new(task_type_keys, Arc::new(task_type_values));

        let operator_values =
            StringArray::from_iter_values(records.iter().map(|r| r.operator.as_str()));
        let operator_keys = UInt8Array::from_iter_values(0..records.len() as u8);
        let operator_array = DictionaryArray::new(operator_keys, Arc::new(operator_values));

        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );
        let start_time_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.start_time.timestamp_nanos_opt().unwrap_or_default()),
        );
        let end_time_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.end_time.timestamp_nanos_opt().unwrap_or_default()),
        );

        let stage_array = Int32Array::from_iter_values(records.iter().map(|r| r.stage));
        let passed_array = BooleanArray::from_iter(records.iter().map(|r| r.passed));
        let condition_array = BooleanArray::from_iter(records.iter().map(|r| r.condition));
        let value_array = Float64Array::from_iter_values(records.iter().map(|r| r.value));

        let assertion_array = StringArray::from_iter_values(records.iter().map(|r| r.assertion()));
        let expected_array =
            StringArray::from_iter_values(records.iter().map(|r| r.expected.to_string()));
        let actual_array =
            StringArray::from_iter_values(records.iter().map(|r| r.actual.to_string()));
        let message_array =
            StringArray::from_iter_values(records.iter().map(|r| r.message.as_str()));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(entity_id_array),
                Arc::new(entity_uid_array),
                Arc::new(record_uid_array),
                Arc::new(task_id_array),
                Arc::new(task_type_array),
                Arc::new(operator_array),
                Arc::new(created_at_array),
                Arc::new(start_time_array),
                Arc::new(end_time_array),
                Arc::new(stage_array),
                Arc::new(passed_array),
                Arc::new(condition_array),
                Arc::new(value_array),
                Arc::new(assertion_array),
                Arc::new(expected_array),
                Arc::new(actual_array),
                Arc::new(message_array),
            ],
        )
        .map_err(DataFrameError::ArrowError)?;

        Ok(batch)
    }
}
