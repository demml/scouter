use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::prelude::ParquetReadOptions;
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

    /// Get the storage root path
    fn storage_root(&self) -> String;

    // Add this new required method
    fn get_session_context(&self) -> Result<SessionContext, ScouterError>;

    // Get the table name
    fn table_name(&self) -> String;

    // Get binned SQL
    fn get_binned_sql(
        &self,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> String;

    /// Load storage files into parquet table for querying
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the parquet file (this path should exclude root path)
    /// * `table_name` - The name of the table to register
    ///
    async fn register_data(
        &self,
        path: &Path,
        table_name: &str,
    ) -> Result<SessionContext, ScouterError> {
        let ctx = self.get_session_context()?;

        let full_path = format!("{}/{}", self.storage_root(), path.display());

        ctx.register_parquet(table_name, full_path, ParquetReadOptions::default())
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to register parquet: {}", e)))?;
        Ok(ctx)
    }

    /// Get binned metrics from the parquet file
    ///
    /// # Arguments
    ///     
    /// * `path` - The path to the parquet file (this path should exclude root path)
    /// * `bin` - The bin value
    /// * `start_time` - The start time to filter
    /// * `end_time` - The end time to filter
    /// * `space` - The space to filter
    /// * `name` - The name to filter
    /// * `version` - The version to filter
    ///
    #[allow(clippy::too_many_arguments)]
    async fn get_binned_metrics(
        &self,
        path: &Path,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> Result<(), ScouterError> {
        // Register the data at path
        let ctx = self.register_data(path, &self.table_name()).await?;

        let sql = self.get_binned_sql(bin, start_time, end_time, space, name, version);
        let df = ctx
            .sql(&sql)
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to read batches: {}", e)))?;

        df.show()
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to show dataframe: {}", e)))?;

        Ok(())
    }
}
