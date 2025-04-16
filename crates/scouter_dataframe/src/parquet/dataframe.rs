use crate::parquet::custom::CustomMetricDataFrame;
use crate::parquet::psi::PsiDataFrame;
use crate::parquet::traits::ParquetFrame;
use crate::storage::ObjectStore;
use chrono::{DateTime, Utc};
use scouter_error::ScouterError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::DriftType;
use scouter_types::ServerRecords;
use std::path::Path;

use crate::parquet::spc::SpcDataFrame;

pub enum ParquetDataFrame {
    CustomMetric(CustomMetricDataFrame),
    Psi(PsiDataFrame),
    Spc(SpcDataFrame),
}

impl ParquetDataFrame {
    pub fn new(
        storage_settings: &ObjectStorageSettings,
        drift_type: &DriftType,
    ) -> Result<Self, ScouterError> {
        match drift_type {
            DriftType::Custom => Ok(ParquetDataFrame::CustomMetric(CustomMetricDataFrame::new(
                storage_settings,
            )?)),
            DriftType::Psi => Ok(ParquetDataFrame::Psi(PsiDataFrame::new(storage_settings)?)),
            DriftType::Spc => Ok(ParquetDataFrame::Spc(SpcDataFrame::new(storage_settings)?)),
        }
    }

    /// Write the records to a parquet file at the given path.
    ///
    /// # Arguments
    ///
    /// * `rpath` - The path to write the parquet file to. (This path should exclude root path)
    /// * `records` - The records to write to the parquet file.
    ///
    pub async fn write_parquet(
        &self,
        rpath: &Path,
        records: ServerRecords,
    ) -> Result<(), ScouterError> {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::Psi(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::Spc(df) => df.write_parquet(rpath, records).await,
        }
    }

    /// primarily used for dev
    pub fn storage_client(&self) -> ObjectStore {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.object_store.clone(),
            ParquetDataFrame::Psi(df) => df.object_store.clone(),
            ParquetDataFrame::Spc(df) => df.object_store.clone(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_binned_metrics(
        &self,
        path: &Path,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> Result<(), ScouterError> {
        match self {
            ParquetDataFrame::CustomMetric(df) => {
                df.get_binned_metrics(path, bin, start_time, end_time, space, name, version)
                    .await?;
                Ok(())
            }
            ParquetDataFrame::Psi(df) => {
                df.get_binned_metrics(path, bin, start_time, end_time, space, name, version)
                    .await?;
                Ok(())
            }
            ParquetDataFrame::Spc(df) => {
                df.get_binned_metrics(path, bin, start_time, end_time, space, name, version)
                    .await?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use super::*;
    use chrono::Utc;
    use object_store::path::Path;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::{
        CustomMetricServerRecord, PsiServerRecord, ServerRecord, ServerRecords, SpcServerRecord,
    };

    fn cleanup() {
        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_write_custom_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &DriftType::Custom).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();

        for i in 0..3 {
            let record = ServerRecord::Custom(CustomMetricServerRecord {
                created_at: Utc::now(),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                metric: "metric".to_string(),
                value: i as f64,
            });

            // sleep 1 second
            std::thread::sleep(std::time::Duration::from_secs(1));
            batch.push(record);
        }

        let records = ServerRecords::new(batch);
        let rpath = PathBuf::from("test.parquet");
        df.write_parquet(&rpath, records).await.unwrap();

        // Check if the file exists
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 1);

        // attempt to read the file
        let read_df = df
            .get_binned_metrics(
                &rpath,
                &0.01,
                &start_utc,
                &Utc::now(),
                "test",
                "test",
                "1.0",
            )
            .await;
        read_df.unwrap();

        // delete the file
        let file_path = files.first().unwrap().to_string();
        let path = Path::from(file_path);
        df.storage_client()
            .delete(&path)
            .await
            .expect("Failed to delete file");

        // Check if the file is deleted
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_psi_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &DriftType::Psi).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();

        for i in 0..2 {
            let record = ServerRecord::Psi(PsiServerRecord {
                created_at: Utc::now(),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                feature: "feature".to_string(),
                bin_id: i as usize,
                bin_count: 10,
            });

            // sleep 1 second
            std::thread::sleep(std::time::Duration::from_secs(1));
            batch.push(record);
        }

        let records = ServerRecords::new(batch);
        let rpath = PathBuf::from("test.parquet");
        df.write_parquet(&rpath, records).await.unwrap();

        // Check if the file exists
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 1);

        let read_df = df
            .get_binned_metrics(
                &rpath,
                &0.01,
                &start_utc,
                &Utc::now(),
                "test",
                "test",
                "1.0",
            )
            .await;
        read_df.unwrap();

        // delete the file
        let file_path = files.first().unwrap().to_string();
        let path = Path::from(file_path);
        df.storage_client()
            .delete(&path)
            .await
            .expect("Failed to delete file");

        // Check if the file is deleted
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }

    #[tokio::test]
    async fn test_write_spc_dataframe_local() {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &DriftType::Spc).unwrap();
        let mut batch = Vec::new();
        let start_utc = Utc::now();

        for i in 0..2 {
            let record = ServerRecord::Spc(SpcServerRecord {
                created_at: Utc::now(),
                name: "test".to_string(),
                space: "test".to_string(),
                version: "1.0".to_string(),
                feature: "feature".to_string(),
                value: i as f64,
            });
            // sleep 1 second
            std::thread::sleep(std::time::Duration::from_secs(1));
            batch.push(record);
        }

        let records = ServerRecords::new(batch);
        let rpath = PathBuf::from("test.parquet");
        df.write_parquet(&rpath, records).await.unwrap();

        // Check if the file exists
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 1);

        let read_df = df
            .get_binned_metrics(
                &rpath,
                &0.01,
                &start_utc,
                &Utc::now(),
                "test",
                "test",
                "1.0",
            )
            .await;
        read_df.unwrap();

        // delete the file
        let file_path = files.first().unwrap().to_string();
        let path = Path::from(file_path);
        df.storage_client()
            .delete(&path)
            .await
            .expect("Failed to delete file");

        // Check if the file is deleted
        let files = df.storage_client().list(None).await.unwrap();
        assert_eq!(files.len(), 0);

        // cleanup
        cleanup();
    }
}
