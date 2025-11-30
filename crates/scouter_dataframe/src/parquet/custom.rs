use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::sql::helper::get_binned_custom_metric_values_query;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow_array::{Int32Array, RecordBatch};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;

use scouter_types::{
    CustomMetricInternalRecord, InternalServerRecords, StorageType, ToInternalDriftRecords,
};
use std::sync::Arc;

pub struct CustomMetricDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for CustomMetricDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        CustomMetricDataFrame::new(storage_settings)
    }

    async fn get_dataframe(
        &self,
        records: InternalServerRecords,
    ) -> Result<DataFrame, DataFrameError> {
        let records = records.to_custom_metric_drift_records()?;
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
        get_binned_custom_metric_values_query(bin, start_time, end_time, entity_id)
    }

    fn table_name(&self) -> String {
        BinnedTableName::CustomMetric.to_string()
    }
}

impl CustomMetricDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("entity_id", DataType::Int32, false),
            Field::new("metric", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(CustomMetricDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(
        &self,
        records: Vec<CustomMetricInternalRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );
        let entity_id_array = Int32Array::from_iter_values(records.iter().map(|r| r.entity_id));
        let metric_array = StringArray::from_iter_values(records.iter().map(|r| r.metric.as_str()));
        let value_array = Float64Array::from_iter_values(records.iter().map(|r| r.value));
        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(entity_id_array),
                Arc::new(metric_array),
                Arc::new(value_array),
            ],
        )?;

        Ok(batch)
    }
}
