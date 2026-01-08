use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::sql::helper::get_binned_genai_task_values_query;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, Int32Array, StringArray, TimestampNanosecondArray};
use arrow_array::{BooleanArray, RecordBatch};
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
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("record_uid", DataType::Utf8, false),
            Field::new("entity_id", DataType::Int32, false),
            Field::new("task_id", DataType::Utf8, false),
            Field::new("task_type", DataType::Utf8, false),
            Field::new("passed", DataType::Boolean, false),
            Field::new("value", DataType::Float64, false),
            Field::new("field_path", DataType::Utf8, true),
            Field::new("operator", DataType::Utf8, false),
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
        // 1. created_at: TimestampNanosecond
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        // 2. record_uid: Utf8
        let uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.record_uid.as_str()));

        // 3. entity_id: Int32
        let entity_id_array = Int32Array::from_iter_values(records.iter().map(|r| r.entity_id));

        // 4. task_id: Utf8
        let task_id_array =
            StringArray::from_iter_values(records.iter().map(|r| r.task_id.as_str()));

        // 5. task_type: Utf8 (Enum to string)
        let task_type_array =
            StringArray::from_iter_values(records.iter().map(|r| r.task_type.as_str()));

        // 6. passed: Boolean
        let passed_array = BooleanArray::from_iter(records.iter().map(|r| r.passed));

        // 7. value: Float64
        let value_array = Float64Array::from_iter_values(records.iter().map(|r| r.value));

        // 8. field_path: Utf8 (Nullable)
        let field_path_array =
            StringArray::from_iter(records.iter().map(|r| r.field_path.as_deref()));

        // 9. operator: Utf8 (Enum to string)
        let operator_array =
            StringArray::from_iter_values(records.iter().map(|r| r.operator.as_str()));

        // 10. expected: Utf8 (JSON to String)
        let expected_array =
            StringArray::from_iter_values(records.iter().map(|r| r.expected.to_string()));

        // 11. actual: Utf8 (JSON to String)
        let actual_array =
            StringArray::from_iter_values(records.iter().map(|r| r.actual.to_string()));

        // 12. message: Utf8
        let message_array =
            StringArray::from_iter_values(records.iter().map(|r| r.message.as_str()));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array), // 1
                Arc::new(uid_array),        // 2
                Arc::new(entity_id_array),  // 3
                Arc::new(task_id_array),    // 4
                Arc::new(task_type_array),  // 5
                Arc::new(passed_array),     // 6
                Arc::new(value_array),      // 7
                Arc::new(field_path_array), // 8
                Arc::new(operator_array),   // 9
                Arc::new(expected_array),   // 10
                Arc::new(actual_array),     // 11
                Arc::new(message_array),    // 12
            ],
        )
        .map_err(DataFrameError::ArrowError)?;

        Ok(batch)
    }
}
