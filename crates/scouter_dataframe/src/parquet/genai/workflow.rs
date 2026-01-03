use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::sql::helper::get_binned_genai_workflow_values_query;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, Int32Array, StringArray, TimestampNanosecondArray};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{GenAIEvalWorkflowRecord, ServerRecords, StorageType, ToDriftRecords};
use std::sync::Arc;

pub struct GenAIWorkflowDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for GenAIWorkflowDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        GenAIWorkflowDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        // Assuming ServerRecords has a method to_genai_workflow_records()
        let records = records.to_genai_workflow_records()?;
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
        // You'll need to implement this helper for workflows specifically
        get_binned_genai_workflow_values_query(bin, start_time, end_time, entity_id)
    }

    fn table_name(&self) -> String {
        // Ensure this variant exists in your BinnedTableName enum
        BinnedTableName::GenAIWorkflow.to_string()
    }
}

impl GenAIWorkflowDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("record_uid", DataType::Utf8, false),
            Field::new("entity_id", DataType::Int32, false),
            Field::new("total_tasks", DataType::Int32, false),
            Field::new("passed_tasks", DataType::Int32, false),
            Field::new("failed_tasks", DataType::Int32, false),
            Field::new("pass_rate", DataType::Float64, false),
            Field::new("duration_ms", DataType::Int64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(GenAIWorkflowDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(
        &self,
        records: Vec<&GenAIEvalWorkflowRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        // 1. created_at
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        // 2. record_uid
        let uid_array =
            StringArray::from_iter_values(records.iter().map(|r| r.record_uid.as_str()));

        // 3. entity_id
        let entity_id_array = Int32Array::from_iter_values(records.iter().map(|r| r.entity_id));

        // 4. total_tasks
        let total_tasks_array = Int32Array::from_iter_values(records.iter().map(|r| r.total_tasks));

        // 5. passed_tasks
        let passed_tasks_array =
            Int32Array::from_iter_values(records.iter().map(|r| r.passed_tasks));

        // 6. failed_tasks
        let failed_tasks_array =
            Int32Array::from_iter_values(records.iter().map(|r| r.failed_tasks));

        // 7. pass_rate
        let pass_rate_array = Float64Array::from_iter_values(records.iter().map(|r| r.pass_rate));

        // 8. duration_ms
        let duration_ms_array =
            arrow_array::Int32Array::from_iter_values(records.iter().map(|r| r.duration_ms));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(uid_array),
                Arc::new(entity_id_array),
                Arc::new(total_tasks_array),
                Arc::new(passed_tasks_array),
                Arc::new(failed_tasks_array),
                Arc::new(pass_rate_array),
                Arc::new(duration_ms_array),
            ],
        )
        .map_err(DataFrameError::ArrowError)?;

        Ok(batch)
    }
}
