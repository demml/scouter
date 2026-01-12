use crate::error::DataFrameError;
use crate::parquet::custom::CustomMetricDataFrame;
use crate::parquet::genai::{GenAIEvalDataFrame, GenAITaskDataFrame, GenAIWorkflowDataFrame};
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
    GenAITask(GenAITaskDataFrame),
    GenAIWorkflow(GenAIWorkflowDataFrame),
    GenAIEval(GenAIEvalDataFrame),
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
            RecordType::GenAITask => Ok(ParquetDataFrame::GenAITask(GenAITaskDataFrame::new(
                storage_settings,
            )?)),
            RecordType::GenAIWorkflow => Ok(ParquetDataFrame::GenAIWorkflow(
                GenAIWorkflowDataFrame::new(storage_settings)?,
            )),
            RecordType::GenAIEval => Ok(ParquetDataFrame::GenAIEval(GenAIEvalDataFrame::new(
                storage_settings,
            )?)),

            _ => Err(DataFrameError::InvalidRecordTypeError(
                record_type.to_string(),
            )),
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
            ParquetDataFrame::GenAITask(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::GenAIWorkflow(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::GenAIEval(df) => df.write_parquet(rpath, records).await,
        }
    }

    pub fn storage_root(&self) -> String {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.storage_root(),
            ParquetDataFrame::Psi(df) => df.storage_root(),
            ParquetDataFrame::Spc(df) => df.storage_root(),
            ParquetDataFrame::GenAITask(df) => df.storage_root(),
            ParquetDataFrame::GenAIWorkflow(df) => df.storage_root(),
            ParquetDataFrame::GenAIEval(df) => df.storage_root(),
        }
    }

    /// primarily used for dev
    pub fn storage_client(&self) -> ObjectStore {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.object_store.clone(),
            ParquetDataFrame::Psi(df) => df.object_store.clone(),
            ParquetDataFrame::Spc(df) => df.object_store.clone(),
            ParquetDataFrame::GenAITask(df) => df.object_store.clone(),
            ParquetDataFrame::GenAIWorkflow(df) => df.object_store.clone(),
            ParquetDataFrame::GenAIEval(df) => df.object_store.clone(),
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
        entity_id: &i32,
    ) -> Result<DataFrame, DataFrameError> {
        let read_path = &self.resolve_path(path);

        match self {
            ParquetDataFrame::CustomMetric(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, entity_id)
                    .await
            }
            ParquetDataFrame::Psi(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, entity_id)
                    .await
            }
            ParquetDataFrame::Spc(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, entity_id)
                    .await
            }

            ParquetDataFrame::GenAITask(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, entity_id)
                    .await
            }
            ParquetDataFrame::GenAIWorkflow(df) => {
                df.get_binned_metrics(read_path, bin, start_time, end_time, entity_id)
                    .await
            }
            ParquetDataFrame::GenAIEval(_) => Err(DataFrameError::UnsupportedOperation(
                "GenAI drift does not support binned metrics".to_string(),
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
            ParquetDataFrame::GenAITask(df) => {
                df.object_store.storage_settings.storage_type.clone()
            }
            ParquetDataFrame::GenAIWorkflow(df) => {
                df.object_store.storage_settings.storage_type.clone()
            }
            ParquetDataFrame::GenAIEval(df) => {
                df.object_store.storage_settings.storage_type.clone()
            }
        }
    }

    pub fn resolve_path(&self, path: &str) -> String {
        format!("{}/{}/", self.storage_root(), path)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::parquet::psi::dataframe_to_psi_drift_features;
    use crate::parquet::spc::dataframe_to_spc_drift_features;
    use crate::parquet::types::BinnedTableName;
    use crate::parquet::utils::BinnedMetricsExtractor;
    use chrono::Utc;
    use object_store::path::Path;
    use rand::Rng;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::{
        BoxedGenAIEvalRecord, GenAIEvalRecord, PsiRecord, ServerRecord, ServerRecords, SpcRecord,
        Status,
    };
    use scouter_types::{CustomMetricRecord, GenAIEvalTaskResult, GenAIEvalWorkflowResult};
    use serde_json::Map;
    use serde_json::Value;

    fn cleanup() {
        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_write_genai_event_record_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::GenAIEval).unwrap();
        let mut batch = Vec::new();
        let entity_id = rand::rng().random_range(0..100);

        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = GenAIEvalRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    entity_id,
                    context: serde_json::Value::Object(Map::new()),
                    status: Status::Pending,
                    id: 0,
                    uid: format!("record_uid_{i}_{j}"),
                    entity_uid: format!("entity_uid_{entity_id}"),
                    ..Default::default()
                };

                let boxed_record = BoxedGenAIEvalRecord::new(record);
                batch.push(ServerRecord::GenAIEval(boxed_record));
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = BinnedTableName::GenAIEval.to_string();
        df.write_parquet(&rpath, records.clone()).await.unwrap();

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
    async fn test_write_genai_task_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::GenAITask).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);
        let entity_id = rand::rng().random_range(0..100);

        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = ServerRecord::GenAITaskRecord(GenAIEvalTaskResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    entity_id,
                    task_id: format!("task{i}"),
                    task_type: scouter_types::genai::EvaluationTaskType::Assertion,
                    passed: true,
                    value: j as f64,
                    field_path: Some(format!("field.path.{i}")),
                    operator: scouter_types::genai::ComparisonOperator::Contains,
                    expected: Value::Null,
                    actual: Value::Null,
                    message: "All good".to_string(),
                    entity_uid: format!("entity_uid_{entity_id}"),
                    condition: false,
                    stage: 0,
                });

                batch.push(record);
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = BinnedTableName::GenAITask.to_string();
        df.write_parquet(&rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        // attempt to read the file
        let new_df = ParquetDataFrame::new(&storage_settings, &RecordType::GenAITask).unwrap();

        let read_df = new_df
            .get_binned_metrics(&rpath, &0.01, &start_utc, &end_utc_for_test, &entity_id)
            .await
            .unwrap();

        //read_df.show().await.unwrap();

        let binned_metrics = BinnedMetricsExtractor::dataframe_to_binned_metrics(read_df)
            .await
            .unwrap();

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
    async fn test_write_genai_workflow_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &RecordType::GenAITask).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();
        let end_utc_for_test = start_utc + chrono::Duration::hours(3);
        let entity_id = rand::rng().random_range(0..100);

        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = ServerRecord::GenAIWorkflowRecord(GenAIEvalWorkflowResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    entity_id,
                    total_tasks: 10,
                    passed_tasks: 8,
                    failed_tasks: 2,
                    pass_rate: 0.8,
                    duration_ms: 1500,
                    entity_uid: format!("entity_uid_{entity_id}"),
                });

                batch.push(record);
            }
        }

        let records = ServerRecords::new(batch);
        let rpath = BinnedTableName::GenAIWorkflow.to_string();
        df.write_parquet(&rpath, records.clone()).await.unwrap();

        // get canonical path
        let canonical_path = df.storage_root();
        let data_path = object_store::path::Path::from(canonical_path);

        // Check if the file exists
        let files = df.storage_client().list(Some(&data_path)).await.unwrap();
        assert_eq!(files.len(), 3);

        // attempt to read the file
        let new_df = ParquetDataFrame::new(&storage_settings, &RecordType::GenAIWorkflow).unwrap();

        let read_df = new_df
            .get_binned_metrics(&rpath, &0.01, &start_utc, &end_utc_for_test, &entity_id)
            .await
            .unwrap();

        //read_df.show().await.unwrap();

        let binned_metrics = BinnedMetricsExtractor::dataframe_to_binned_metrics(read_df)
            .await
            .unwrap();

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
        let entity_id = rand::rng().random_range(0..100);
        // create records
        for i in 0..3 {
            for j in 0..50 {
                let record = ServerRecord::Custom(CustomMetricRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    metric: format!("metric{i}"),
                    value: j as f64,
                    entity_id,
                    uid: format!("entity_uid_{entity_id}"),
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
            .get_binned_metrics(rpath, &0.01, &start_utc, &end_utc_for_test, &entity_id)
            .await
            .unwrap();

        //read_df.show().await.unwrap();

        let binned_metrics = BinnedMetricsExtractor::dataframe_to_binned_metrics(read_df)
            .await
            .unwrap();

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
        let entity_id = rand::rng().random_range(0..100);
        for i in 0..3 {
            for j in 0..5 {
                let record = ServerRecord::Psi(PsiRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    feature: "feature1".to_string(),
                    bin_id: j,
                    bin_count: rand::rng().random_range(0..100),
                    entity_id,
                    uid: format!("entity_uid_{entity_id}"),
                });

                batch.push(record);
            }
        }

        for i in 0..3 {
            for j in 0..5 {
                let record = ServerRecord::Psi(PsiRecord {
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    feature: "feature2".to_string(),
                    bin_id: j,
                    bin_count: rand::rng().random_range(0..100),
                    entity_id,
                    uid: format!("entity_uid_{entity_id}"),
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
            .get_binned_metrics(rpath, &0.01, &start_utc, &end_utc_for_test, &entity_id)
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
        let entity_id = rand::rng().random_range(0..100);
        for i in 0..5 {
            let record = ServerRecord::Spc(SpcRecord {
                created_at: Utc::now() + chrono::Duration::hours(i),
                feature: "feature1".to_string(),
                value: i as f64,
                entity_id,
                uid: format!("entity_uid_{entity_id}"),
            });

            batch.push(record);
        }

        for i in 0..5 {
            let record = ServerRecord::Spc(SpcRecord {
                created_at: Utc::now() + chrono::Duration::hours(i),
                feature: "feature2".to_string(),
                value: i as f64,
                entity_id,
                uid: format!("entity_uid_{entity_id}"),
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
            .get_binned_metrics(rpath, &0.01, &start_utc, &end_utc_for_test, &entity_id)
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
