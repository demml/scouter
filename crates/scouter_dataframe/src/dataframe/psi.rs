use crate::dataframe::traits::ParquetFrame;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{StringArray, TimestampNanosecondArray, UInt64Array};
use arrow_array::RecordBatch;
use async_trait::async_trait;
use datafusion::dataframe::DataFrame;
use datafusion::dataframe::DataFrameWriteOptions;
use scouter_error::ScouterError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::ToDriftRecords;
use scouter_types::{PsiServerRecord, ServerRecords};
use std::path::Path;
use std::sync::Arc;
pub struct PsiDataFrame {
    schema: Arc<Schema>,
    object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for PsiDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError> {
        PsiDataFrame::new(storage_settings)
    }

    /// Write the records to a parquet file at the given path.
    /// The path should be a valid URL for the object store.
    ///
    /// # Arguments
    ///
    /// * `rpath` - The path to write the parquet file to.
    /// * `records` - The records to write to the parquet file.
    ///
    async fn write_parquet(
        &self,
        rpath: &Path,
        records: ServerRecords,
    ) -> Result<(), ScouterError> {
        let records = records.to_psi_drift_records()?;
        let df = self.get_dataframe(records).await?;

        let str_path = rpath
            .as_os_str()
            .to_str()
            .ok_or_else(|| ScouterError::Error("Invalid path".to_string()))?;

        df.write_parquet(str_path, DataFrameWriteOptions::new(), None)
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to write parquet: {}", e)))?;

        Ok(())
    }
}

impl PsiDataFrame {
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
            Field::new("bin_id", DataType::UInt64, false),
            Field::new("bin_count", DataType::UInt64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(PsiDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(&self, records: Vec<PsiServerRecord>) -> Result<RecordBatch, ScouterError> {
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        let space_array = StringArray::from_iter_values(records.iter().map(|r| r.space.as_str()));
        let name_array = StringArray::from_iter_values(records.iter().map(|r| r.name.as_str()));
        let version_array =
            StringArray::from_iter_values(records.iter().map(|r| r.version.as_str()));
        let feature_array =
            StringArray::from_iter_values(records.iter().map(|r| r.feature.as_str()));

        let bin_id_array = UInt64Array::from_iter_values(records.iter().map(|r| r.bin_id as u64));
        let bin_count_array =
            UInt64Array::from_iter_values(records.iter().map(|r| r.bin_count as u64));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(space_array),
                Arc::new(name_array),
                Arc::new(version_array),
                Arc::new(feature_array),
                Arc::new(bin_id_array),
                Arc::new(bin_count_array),
            ],
        )
        .map_err(|e| ScouterError::Error(format!("Failed to create RecordBatch: {}", e)))?;

        Ok(batch)
    }

    async fn get_dataframe(
        &self,
        records: Vec<PsiServerRecord>,
    ) -> Result<DataFrame, ScouterError> {
        let batch = self.build_batch(records)?;

        let ctx = self.object_store.get_session()?;

        let df = ctx
            .read_batches(vec![batch])
            .map_err(|e| ScouterError::Error(format!("Failed to read batches: {}", e)))?;
        Ok(df)
    }
}
