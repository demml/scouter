use async_trait::async_trait;
use scouter_error::ScouterError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::ServerRecords;
use std::path::Path;

#[async_trait]
pub trait ParquetFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, ScouterError>
    where
        Self: Sized;

    async fn write_parquet(&self, rpath: &Path, records: ServerRecords)
        -> Result<(), ScouterError>;
}
