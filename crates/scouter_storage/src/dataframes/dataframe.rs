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

#[async_trait]
impl ParquetFrame for CustomMetricDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError> {
        CustomMetricDataFrame::new(storage_settings)
    }

    async fn write_parquet(&self, rpath: &str, records: ServerRecords) -> Result<(), ScouterError> {
        let records = records.to_custom_metric_drift_records()?;
        self.write_custom(rpath, records).await?;
        Ok(())
    }
}

impl CustomMetricDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("name", DataType::Utf8, false),
            Field::new("space", DataType::Utf8, false),
            Field::new("version", DataType::Utf8, false),
            Field::new("feature", DataType::Utf8, false),
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
        records: Vec<CustomMetricServerRecord>,
    ) -> Result<RecordBatch, ScouterError> {
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        let space_array = StringArray::from_iter_values(records.iter().map(|r| r.space.as_str()));
        let name_array = StringArray::from_iter_values(records.iter().map(|r| r.name.as_str()));
        let version_array =
            StringArray::from_iter_values(records.iter().map(|r| r.version.as_str()));
        let metric_array = StringArray::from_iter_values(records.iter().map(|r| r.metric.as_str()));

        let value_array = Float64Array::from_iter_values(records.iter().map(|r| r.value));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(space_array),
                Arc::new(name_array),
                Arc::new(version_array),
                Arc::new(metric_array),
                Arc::new(value_array),
            ],
        )
        .map_err(|e| ScouterError::Error(format!("Failed to create RecordBatch: {}", e)))?;

        Ok(batch)
    }

    async fn get_dataframe(
        &self,
        records: Vec<CustomMetricServerRecord>,
    ) -> Result<DataFrame, ScouterError> {
        let batch = self.build_batch(records)?;

        let table = MemTable::try_new(self.schema.clone(), vec![vec![batch]])
            .map_err(|e| ScouterError::Error(format!("Failed to create MemTable: {}", e)))?;

        let ctx = self.object_store.get_session()?;

        ctx.register_table("metrics", Arc::new(table))
            .map_err(|e| ScouterError::Error(format!("Failed to register table: {}", e)))?;

        // Execute a query to verify
        let df = ctx
            .sql("SELECT * FROM metrics")
            .await
            .map_err(|_| StorageError::StorageError("Failed to execute query".to_string()))?;

        Ok(df)
    }
    pub async fn write_custom(
        &self,
        rpath: &str,
        records: Vec<CustomMetricServerRecord>,
    ) -> Result<(), ScouterError> {
        let df = self.get_dataframe(records).await?;

        df.write_parquet(rpath, DataFrameWriteOptions::new(), None)
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to write parquet: {}", e)))?;

        Ok(())
    }
}

// testing

#[cfg(test)]
mod tests {

    use super::*;
    use chrono::Utc;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::ServerRecord;

    #[tokio::test]
    async fn test_write_custom_dataframe() {
        let storage_settings = ObjectStorageSettings::default();
        let df = CustomMetricDataFrame::new(&storage_settings).unwrap();
        let mut batch = Vec::new();

        for i in 0..10 {
            let record = ServerRecord::Custom(CustomMetricServerRecord {
                created_at: Utc::now(),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                metric: "metric".to_string(),
                value: i as f64,
            });

            batch.push(record);
        }

        let records = ServerRecords::new(batch);
        let rpath = "test.parquet";

        df.write_parquet(rpath, records).await.unwrap();
    }
}
