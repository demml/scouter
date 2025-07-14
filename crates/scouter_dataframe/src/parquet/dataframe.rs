use crate::error::DataFrameError;
use crate::parquet::custom::CustomMetricDataFrame;
use crate::parquet::llm::{LLMDriftDataFrame, LLMMetricDataFrame};
use crate::parquet::psi::PsiDataFrame;
use crate::parquet::spc::SpcDataFrame;
use crate::parquet::traits::ParquetFrame;
use crate::storage::ObjectStore;
use chrono::{DateTime, Utc};
use datafusion::prelude::DataFrame;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{RecordType, ServerRecords, StorageType};
use tracing::instrument;

pub enum ParquetDataFrame {
    CustomMetric(CustomMetricDataFrame),
    Psi(PsiDataFrame),
    Spc(SpcDataFrame),
    LLMMetric(LLMMetricDataFrame),
    LLMDrift(LLMDriftDataFrame),
}

impl ParquetDataFrame {
    pub fn new(
        storage_settings: &ObjectStorageSettings,
        record_type: &RecordType,
    ) -> Result<Self, DataFrameError> {
        match record_type {
            RecordType::Custom => Ok(ParquetDataFrame::CustomMetric(CustomMetricDataFrame::new(
                storage_settings,
            )?)),
            RecordType::Psi => Ok(ParquetDataFrame::Psi(PsiDataFrame::new(storage_settings)?)),
            RecordType::Spc => Ok(ParquetDataFrame::Spc(SpcDataFrame::new(storage_settings)?)),
            RecordType::LLMMetric => Ok(ParquetDataFrame::LLMMetric(LLMMetricDataFrame::new(
                storage_settings,
            )?)),
            RecordType::LLMDrift => Ok(ParquetDataFrame::LLMDrift(LLMDriftDataFrame::new(
                storage_settings,
            )?)),

            _ => Err(DataFrameError::InvalidRecordTypeError),
        }
    }

    /// Write the records to a parquet file at the given path.
    ///
    /// # Arguments
    ///
    /// * `rpath` - The path to write the parquet file to. (This path should exclude root path)
    /// * `records` - The records to write to the parquet file.
    ///
    #[instrument(skip_all, err)]
    pub async fn write_parquet(
        &self,
        rpath: &str,
        records: ServerRecords,
    ) -> Result<(), DataFrameError> {
        let rpath = &self.resolve_path(rpath);

        match self {
            ParquetDataFrame::CustomMetric(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::Psi(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::Spc(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::LLMMetric(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::LLMDrift(df) => df.write_parquet(rpath, records).await,
        }
    }

    pub fn storage_root(&self) -> String {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.storage_root(),
            ParquetDataFrame::Psi(df) => df.storage_root(),
            ParquetDataFrame::Spc(df) => df.storage_root(),
            ParquetDataFrame::LLMMetric(df) => df.storage_root(),
            ParquetDataFrame::LLMDrift(df) => df.storage_root(),
        }
    }

    /// primarily used for dev
    pub fn storage_client(&self) -> ObjectStore {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.object_store.clone(),
            ParquetDataFrame::Psi(df) => df.object_store.clone(),
            ParquetDataFrame::Spc(df) => df.object_store.clone(),
            ParquetDataFrame::LLMMetric(df) => df.object_store.clone(),
            ParquetDataFrame::LLMDrift(df) => df.object_store.clone(),
        }
    }

    /// Get binned metrics from archived parquet files
    ///
    /// # Arguments
    /// * path - The path to the parquet files (directory). This will be read as a table listing
    /// * bin - The bin size
    /// * start_time - The start time of the query
    /// * end_time - The end time of the query
    /// * space - The space to query
    /// * name - The name to query
    /// * version - The version to query
    #[allow(clippy::too_many_arguments)]
    pub async fn get_binned_metrics(
        &self,
        path: &str,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> Result<DataFrame, DataFrameError> {
        let read_path = &self.resolve_path(path);

        match self {
            ParquetDataFrame::CustomMetric(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, space, name, version)
                    .await
            }
            ParquetDataFrame::Psi(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, space, name, version)
                    .await
            }
            ParquetDataFrame::Spc(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, space, name, version)
                    .await
            }

            ParquetDataFrame::LLMMetric(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, space, name, version)
                    .await
            }
            ParquetDataFrame::LLMDrift(_) => Err(DataFrameError::UnsupportedOperation(
                "LLMDrift does not support binned metrics".to_string(),
            )),
        }
    }

    /// Get the underyling object store storage type
    pub fn storage_type(&self) -> StorageType {
        match self {
            ParquetDataFrame::CustomMetric(df) => {
                df.object_store.storage_settings.storage_type.clone()
            }
            ParquetDataFrame::Psi(df) => df.object_store.storage_settings.storage_type.clone(),
            ParquetDataFrame::Spc(df) => df.object_store.storage_settings.storage_type.clone(),
            ParquetDataFrame::LLMMetric(df) => {
                df.object_store.storage_settings.storage_type.clone()
            }
            ParquetDataFrame::LLMDrift(df) => df.object_store.storage_settings.storage_type.clone(),
        }
    }

    pub fn resolve_path(&self, path: &str) -> String {
        format!("{}/{}/", self.storage_root(), path)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::parquet::custom::dataframe_to_custom_drift_metrics;
    use crate::parquet::llm::metric::dataframe_to_llm_drift_metrics;
    use crate::parquet::psi::dataframe_to_psi_drift_features;
    use crate::parquet::spc::dataframe_to_spc_drift_features;
    use chrono::Utc;
    use object_store::path::Path;
    use potato_head::create_score_prompt;
    use rand::Rng;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::{
        BoxedLLMDriftServerRecord, CustomMetricServerRecord, LLMDriftServerRecord,
        LLMMetricServerRecord, PsiServerRecord, ServerRecord, ServerRecords, SpcServerRecord,
        Status,
    };
    use serde_json::Map;

    fn cleanup() {
        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_write_llm_drift_record_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::LLMDrift).unwrap();
        let mut batch = Vec::new();

        let prompt = create_score_prompt();

        // create records
        for i in 0..3 {
            for _ in 0..50 {
                let record = LLMDriftServerRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    space: "test".to_string(),
                    name: "test".to_string(),
                    version: "1.0".to_string(),
                    prompt: prompt.clone(),
                    input: format!("input{i}"),
                    response: format!("output{i}"),
                    context: serde_json::Value::Object(Map::new()),
                    status: Status::Pending,
                    id: 0,
                    uid: "test-uid".to_string(),
                    updated_at: None,
                    processing_started_at: None,
                    processing_ended_at: None,
                };

                let boxed_record = BoxedLLMDriftServerRecord::new(record);
                batch.push(ServerRecord::LLMDrift(boxed_record));
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = "llm_drift";
        df.write_parquet(rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        //// delete the file
        for file in files.iter() {
            let path = Path::from(file.to_string());
            df.storage_client()
                .delete(&path)
                .await
                .expect("Failed to delete file");
        }
        //
        //// Check if the file is deleted
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_llm_drift_metric_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::LLMMetric).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);

        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = ServerRecord::LLMMetric(LLMMetricServerRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "1.0".to_string(),
                    metric: format!("metric{i}"),
                    value: j as f64,
                });

                batch.push(record);
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = "llm_metric";
        df.write_parquet(rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        // attempt to read the file
        let new_df = ParquetDataFrame::new(&storage_settings, &RecordType::LLMMetric).unwrap();

        let read_df = new_df
            .get_binned_metrics(
                rpath,
                &0.01,
                &start_utc,
                &end_utc_for_test,
                "test",
                "test",
                "1.0",
            )
            .await
            .unwrap();

        //read_df.show().await.unwrap();

        let binned_metrics = dataframe_to_llm_drift_metrics(read_df).await.unwrap();

        assert_eq!(binned_metrics.metrics.len(), 3);

        //// delete the file
        for file in files.iter() {
            let path = Path::from(file.to_string());
            df.storage_client()
                .delete(&path)
                .await
                .expect("Failed to delete file");
        }
        //
        //// Check if the file is deleted
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_custom_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::Custom).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);

        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = ServerRecord::Custom(CustomMetricServerRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "1.0".to_string(),
                    metric: format!("metric{i}"),
                    value: j as f64,
                });

                batch.push(record);
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = "custom";
        df.write_parquet(rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        // attempt to read the file
        let new_df = ParquetDataFrame::new(&storage_settings, &RecordType::Custom).unwrap();

        let read_df = new_df
            .get_binned_metrics(
                rpath,
                &0.01,
                &start_utc,
                &end_utc_for_test,
                "test",
                "test",
                "1.0",
            )
            .await
            .unwrap();

        //read_df.show().await.unwrap();

        let binned_metrics = dataframe_to_custom_drift_metrics(read_df).await.unwrap();

        assert_eq!(binned_metrics.metrics.len(), 3);

        //// delete the file
        for file in files.iter() {
            let path = Path::from(file.to_string());
            df.storage_client()
                .delete(&path)
                .await
                .expect("Failed to delete file");
        }
        //
        //// Check if the file is deleted
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_psi_dataframe_local() {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::Psi).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);

        for i in 0..3 {
            for j in 0..5 {
                let record = ServerRecord::Psi(PsiServerRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "1.0".to_string(),
                    feature: "feature1".to_string(),
                    bin_id: j as usize,
                    bin_count: rand::rng().random_range(0..100),
                });

                batch.push(record);
            }
        }

        for i in 0..3 {
            for j in 0..5 {
                let record = ServerRecord::Psi(PsiServerRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "1.0".to_string(),
                    feature: "feature2".to_string(),
                    bin_id: j as usize,
                    bin_count: rand::rng().random_range(0..100),
                });

                batch.push(record);
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = "psi";
        df.write_parquet(rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        // attempt to read the file
        let read_df = df
            .get_binned_metrics(
                rpath,
                &0.01,
                &start_utc,
                &end_utc_for_test,
                "test",
                "test",
                "1.0",
            )
            .await
            .unwrap();

        let psi_drift = dataframe_to_psi_drift_features(read_df).await.unwrap();
        assert_eq!(psi_drift.len(), 2);

        //// delete the file
        for file in files.iter() {
            let path = Path::from(file.to_string());
            df.storage_client()
                .delete(&path)
                .await
                .expect("Failed to delete file");
        }
        //
        //// Check if the file is deleted
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_spc_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::Spc).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);

        for i in 0..5 {
            let record = ServerRecord::Spc(SpcServerRecord {
                created_at: Utc::now() + chrono::Duration::hours(i),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                feature: "feature1".to_string(),
                value: i as f64,
            });

            batch.push(record);
        }

        for i in 0..5 {
            let record = ServerRecord::Spc(SpcServerRecord {
                created_at: Utc::now() + chrono::Duration::hours(i),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                feature: "feature2".to_string(),
                value: i as f64,
            });

            batch.push(record);
        }

        let records = ServerRecords::new(batch);
        let rpath = "spc";
        df.write_parquet(rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 5);

        // attempt to read the file
        let read_df = df
            .get_binned_metrics(
                rpath,
                &0.01,
                &start_utc,
                &end_utc_for_test,
                "test",
                "test",
                "1.0",
            )
            .await
            .unwrap();

        let _spc_drift = dataframe_to_spc_drift_features(read_df).await.unwrap();

        //// delete the file
        for file in files.iter() {
            let path = Path::from(file.to_string());
            df.storage_client()
                .delete(&path)
                .await
                .expect("Failed to delete file");
        }
        //
        //// Check if the file is deleted
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }
}
