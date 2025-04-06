use async_trait::async_trait;
use datafusion::prelude::SessionContext;
use datafusion::{dataframe::DataFrameWriteOptions, prelude::DataFrame};
use scouter_error::ScouterError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::ServerRecords;
use std::path::Path;

#[async_trait]
pub trait ParquetFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError>
    where
        Self: Sized;

    /// Write the records to a parquet file at the given path.
    ///
    /// # Arguments
    ///
    /// * `rpath` - The path to write the parquet file to. (This path should exclude root path)
    /// * `records` - The records to write to the parquet file.
    ///
    async fn write_parquet(
        &self,
        rpath: &Path,
        records: ServerRecords,
    ) -> Result<(), ScouterError> {
        let df = self.get_dataframe(records).await?;

        let full_rpath = format!("{}/{}", self.storage_root(), rpath.display());

        df.write_parquet(&full_rpath, DataFrameWriteOptions::new(), None)
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to write parquet: {}", e)))?;

        Ok(())
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, ScouterError>;

    fn storage_root(&self) -> String;

    async fn register_data(&self, path: &Path) -> Result<SessionContext, ScouterError>;
}
