use arrow::datatypes::DataType;

use crate::error::DataFrameError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::datasource::file_format::parquet::ParquetFormat;
use datafusion::datasource::listing::{
    ListingOptions, ListingTable, ListingTableConfig, ListingTableUrl,
};
use datafusion::prelude::SessionContext;
use datafusion::prelude::*;
use datafusion::{dataframe::DataFrameWriteOptions, prelude::DataFrame};
use scouter_settings::ObjectStorageSettings;
use scouter_types::ServerRecords;
use scouter_types::StorageType;
use tracing::instrument;

use std::sync::Arc;
#[async_trait]
pub trait ParquetFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError>
    where
        Self: Sized;

    /// Write the records to a parquet file at the given path.
    ///
    /// # Arguments
    ///
    /// * `rpath` - The path to write the parquet file to. (This path should exclude root path)
    /// * `records` - The records to write to the parquet file.
    ///
    #[instrument(skip_all, err)]
    async fn write_parquet(
        &self,
        rpath: &str,
        records: ServerRecords,
    ) -> Result<(), DataFrameError> {
        let df = self.get_dataframe(records).await?;

        // add partition columns
        let df = df
            .with_column("year", date_part(lit("year"), col("created_at")))
            .map_err(DataFrameError::AddYearColumnError)?
            .with_column("month", date_part(lit("month"), col("created_at")))
            .map_err(DataFrameError::AddMonthColumnError)?
            .with_column("day", date_part(lit("day"), col("created_at")))
            .map_err(DataFrameError::AddDayColumnError)?
            .with_column("hour", date_part(lit("hour"), col("created_at")))
            .map_err(DataFrameError::AddHourColumnError)?;

        let write_options = DataFrameWriteOptions::new().with_partition_by(vec![
            // time partitioning
            "year".to_string(),
            "month".to_string(),
            "day".to_string(),
            "hour".to_string(),
        ]);

        df.write_parquet(rpath, write_options, None)
            .await
            .map_err(DataFrameError::WriteParquetError)?;

        Ok(())
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError>;

    /// Get the storage root path
    fn storage_root(&self) -> String;

    fn storage_type(&self) -> StorageType;

    // Add this new required method
    fn get_session_context(&self) -> Result<SessionContext, DataFrameError>;

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
    async fn register_table(
        &self,
        path: &str,
        table_name: &str,
    ) -> Result<SessionContext, DataFrameError> {
        let ctx = self.get_session_context()?;

        let table_path = ListingTableUrl::parse(path)?;

        let file_format = ParquetFormat::new();
        let listing_options = ListingOptions::new(Arc::new(file_format))
            .with_file_extension(".parquet")
            .with_target_partitions(8)
            .with_table_partition_cols(vec![
                ("year".to_string(), DataType::Int32),
                ("month".to_string(), DataType::Int32),
                ("day".to_string(), DataType::Int32),
                ("hour".to_string(), DataType::Int32),
            ]);

        let resolved_schema = listing_options
            .infer_schema(&ctx.state(), &table_path)
            .await
            .map_err(DataFrameError::InferSchemaError)?;

        let config = ListingTableConfig::new(table_path)
            .with_listing_options(listing_options)
            .with_schema(resolved_schema);

        let provider = Arc::new(
            ListingTable::try_new(config).map_err(DataFrameError::CreateListingTableError)?,
        );

        ctx.register_table(table_name, provider)
            .map_err(DataFrameError::RegisterTableError)?;
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
        path: &str,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> Result<DataFrame, DataFrameError> {
        // Register the data at path
        let ctx = self.register_table(path, &self.table_name()).await?;
        let sql = self.get_binned_sql(bin, start_time, end_time, space, name, version);

        let df = ctx.sql(&sql).await?;

        Ok(df)
    }
}
