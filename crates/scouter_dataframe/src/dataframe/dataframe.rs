use crate::dataframe::custom::CustomMetricDataFrame;
use crate::dataframe::psi::PsiDataFrame;
use crate::dataframe::traits::ParquetFrame;
use scouter_error::{ScouterError, StorageError};
use scouter_settings::ObjectStorageSettings;
use scouter_types::DriftType;
use scouter_types::ServerRecords;
use std::path::Path;

pub enum ParquetDataFrame {
    CustomMetric(CustomMetricDataFrame),
    Psi(PsiDataFrame),
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
            _ => Err(StorageError::ObjectStoreError("Unsupported drift type".to_string()).into()),
        }
    }

    pub async fn write_parquet(
        &self,
        rpath: &Path,
        records: ServerRecords,
    ) -> Result<(), ScouterError> {
        match self {
            ParquetDataFrame::CustomMetric(df) => df.write_parquet(rpath, records).await,
            ParquetDataFrame::Psi(df) => df.write_parquet(rpath, records).await,
        }
    }
}

#[cfg(test)]
mod tests {

    use std::path::PathBuf;

    use super::*;
    use chrono::Utc;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::CustomMetricServerRecord;
    use scouter_types::ServerRecord;

    #[tokio::test]
    async fn test_write_custom_dataframe_local() {
        let storage_settings = ObjectStorageSettings::default();
        let df = ParquetDataFrame::new(&storage_settings, &DriftType::Custom).unwrap();
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
        let rpath = PathBuf::from("test.parquet");

        df.write_parquet(&rpath, records).await.unwrap();

        // cleanup
    }
}
