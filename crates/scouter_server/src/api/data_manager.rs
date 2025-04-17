use chrono::{Duration, Utc};
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_error::ScouterError;
/// Functionality for persisting data from postgres to long-term storage
use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
use scouter_sql::{sql::schema::Entity, PostgresClient};
use scouter_types::DriftType;
use scouter_types::RecordType;
use scouter_types::ServerRecords;
use sqlx::{Pool, Postgres};
use strum::IntoEnumIterator;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::span::Record;

pub struct DataManager {
    /// handler for background tasks
    pub workers: Vec<JoinHandle<()>>,
}

impl DataManager {
    pub async fn start_workers(
        pool: &Pool<Postgres>,
        db_settings: &DatabaseSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), EventError> {
        let mut workers = Vec::with_capacity(1);

        let db_client = PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;

        let shutdown_rx = shutdown_rx.clone();
        let worker_shutdown_rx = shutdown_rx.clone();

        Ok(())
    }

    async fn data_archival_worker(
        mut db_client: PostgresClient,
        mut shutdown: watch::Receiver<()>, // Accept receiver
    ) {
        let mut last_cleanup = None;

        loop {
            let now = Utc::now();
            let should_run = match last_cleanup {
                None => true, // Run immediately on first startup
                Some(last_time) => now.signed_duration_since(last_time) >= Duration::days(1),
            };

            if should_run {
                // get dat data
                db_client.get_data_for_archival(record_type, days)

                //match db_client.archive_old_data().await {
                //    Ok(_) => {
                //        last_cleanup = Some(now);
                //    }
                //    Err(e) => {
                //        error!("Data archival error: {:?}", e);
                //    }
                //}
            }
        }
    }
}

async fn get_entities_for_archive(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
) -> Result<Vec<Entity>, ScouterError> {
    // get the data from the database
    let data = db_client
        .get_entities_for_archive(record_type, retention_period)
        .await?;

    Ok(data)
}

async fn get_data_for_archive(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
    entity: &Entity,
) -> Result<ServerRecords, ScouterError> {
    // get the data from the database
    let data = db_client
        .get_data_for_archive(
            retention_period,
            &entity.space,
            &entity.name,
            &entity.version,
            record_type,
        )
        .await?;

    Ok(data)
}

async fn process_record_type(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
    storage_settings: &ObjectStorageSettings,
) -> Result<(), ScouterError> {
    ParquetDataFrame::new(storage_settings, &record_type)?;

    // get the entities for archival
    let entities = get_entities_for_archive(db_client, record_type, retention_period).await?;

    // iterate over the entities and archive the data
    for entity in entities {
        let data = get_data_for_archive(db_client, record_type, retention_period, &entity).await?;

        // archive the data to the object storage
        db_client.archive_data(data).await?;
    }

    Ok(())
}

async fn archive_old_data(
    db_client: &PostgresClient,
    storage_settings: &ObjectStorageSettings,
) -> Result<(), EventError> {
    // get old records
    // iterate of RecordType.Psi, RecordType.Spc, RecordType.Custom
    for record_type in DriftType::iter() {
        // filter out the record types that are not supported

        // get the data from the database
        let data = db_client
            .get_data_for_archival(&record_type, &storage_settings.retention_period)
            .await?;

        // archive the data to the object storage
        storage_settings.archive_data(data).await?;
    }

    Ok(())
}
