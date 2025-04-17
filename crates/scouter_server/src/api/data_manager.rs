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
use std::path::PathBuf;
use strum::IntoEnumIterator;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument};

pub struct DataManager {
    /// handler for background tasks
    pub workers: Vec<JoinHandle<()>>,
}

impl DataManager {
    pub async fn start_workers(
        pool: &Pool<Postgres>,
        db_settings: &DatabaseSettings,
        storage_settings: &ObjectStorageSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ScouterError> {
        let mut workers = Vec::with_capacity(1);

        let db_client = PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;

        let shutdown_rx = shutdown_rx.clone();
        let worker_shutdown_rx = shutdown_rx.clone();
        let retention_period = db_settings.retention_period;
        let storage_settings = storage_settings.clone();

        workers.push(tokio::spawn(Self::start_worker(
            0,
            retention_period,
            storage_settings,
            db_client,
            worker_shutdown_rx,
        )));

        Ok(())
    }

    async fn start_worker(
        id: usize,
        retention_period: i64,
        storage_settings: ObjectStorageSettings,
        db_client: PostgresClient,
        mut shutdown: watch::Receiver<()>,
    ) {
        // pause the worker for 1 hour after it completes
        let mut interval = tokio::time::interval(Duration::hours(1).to_std().unwrap());
        let mut last_cleanup = None;

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Drift executor {}: Shutting down", id);
                    break;
                }
                _ = interval.tick() => {
                    let now = Utc::now();
                    let should_run = match last_cleanup {
                        None => true,
                        Some(last_time) => now.signed_duration_since(last_time) >= Duration::days(1),
                    };

                    if should_run {
                        match archive_old_data(&db_client, &storage_settings, &retention_period).await {
                            Ok(_) => {
                                info!("Archive completed successfully for worker {}", id);
                                last_cleanup = Some(now);
                            }
                            Err(e) => error!("Archive failed for worker {}: {}", id, e),
                        }
                    }
                }
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

async fn update_entities_to_archived(
    db_client: &PostgresClient,
    record_type: &RecordType,
    entity: &Entity,
) -> Result<Vec<Entity>, ScouterError> {
    // get the data from the database
    let data = db_client
        .get_entities_for_archive(record_type, retention_period)
        .await?;

    Ok(data)
}

#[instrument(skip_all)]
async fn process_record_type(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
    storage_settings: &ObjectStorageSettings,
) -> Result<(), ScouterError> {
    info!("Archiving data for record type: {:?}", record_type);
    let df = ParquetDataFrame::new(storage_settings, &record_type)?;

    // get the entities for archival
    let entities = get_entities_for_archive(db_client, record_type, retention_period).await?;

    // exit if no entities
    if entities.is_empty() {
        info!("No entities found for record type: {:?}", record_type);
        return Ok(());
    }

    // iterate over the entities and archive the data
    for entity in entities {
        let records =
            get_data_for_archive(db_client, record_type, retention_period, &entity).await?;

        // get created at as YYYY-MM-DD string
        let created_at = entity.created_at.format("%Y-%m-%d").to_string();

        // archive the data to the object storage
        let rpath = format!(
            "{}/{}/{}/{}/{}",
            created_at, entity.space, entity.name, entity.version, record_type
        );
        df.write_parquet(&PathBuf::from(rpath), records).await?;

        // update the entity in the database
    }

    info!("Archiving data for record type: {:?} complete", record_type);

    Ok(())
}

/// Parent function used to archive old data
///
/// # Arguments
/// * `db_client` - The database client to use for the archival
/// * `storage_settings` - The storage settings to use for the archival
/// * `retention_period` - The retention period to use for the archival
///
/// # Returns
/// * `Result<(), ScouterError>` - The result of the archival
#[instrument(skip_all)]
async fn archive_old_data(
    db_client: &PostgresClient,
    storage_settings: &ObjectStorageSettings,
    retention_period: &i64,
) -> Result<(), ScouterError> {
    // get old records
    // iterate of RecordType.Psi, RecordType.Spc, RecordType.Custom
    info!("Archiving old data");
    for drift_type in DriftType::iter() {
        match drift_type {
            DriftType::Psi => {
                // get the data from the database
                process_record_type(
                    db_client,
                    &RecordType::Psi,
                    retention_period,
                    storage_settings,
                )
                .await?;
            }
            DriftType::Spc => {
                // get the data from the database
                process_record_type(
                    db_client,
                    &RecordType::Spc,
                    retention_period,
                    storage_settings,
                )
                .await?;
            }
            DriftType::Custom => {
                // get the data from the database
                process_record_type(
                    db_client,
                    &RecordType::Custom,
                    retention_period,
                    storage_settings,
                )
                .await?;
            }
        }
    }

    Ok(())
}
