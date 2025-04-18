use chrono::{Duration, Utc};
use rand::Rng;
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_error::ScouterError;
/// Functionality for persisting data from postgres to long-term storage
use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
use scouter_sql::{sql::schema::Entity, PostgresClient};
use scouter_types::DriftType;
use scouter_types::RecordType;
use scouter_types::ServerRecords;
use sqlx::Transaction;
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

async fn get_entities_to_archive(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
) -> Result<Vec<Entity>, ScouterError> {
    // get the data from the database
    let data = db_client
        .get_entities_to_archive(record_type, retention_period)
        .await?;

    Ok(data)
}

async fn get_data_to_archive(
    tx: &mut Transaction<'_, Postgres>,
    record_type: &RecordType,
    entity: &Entity,
) -> Result<ServerRecords, ScouterError> {
    // get the data from the database
    let data = PostgresClient::get_data_to_archive(
        &entity.space,
        &entity.name,
        &entity.version,
        &entity.begin_timestamp,
        &entity.end_timestamp,
        record_type,
        tx,
    )
    .await?;

    Ok(data)
}

async fn update_entities_to_archived(
    tx: &mut Transaction<'_, Postgres>,
    record_type: &RecordType,
    entity: &Entity,
) -> Result<(), ScouterError> {
    // get the data from the database
    PostgresClient::update_data_to_archived(
        &entity.space,
        &entity.name,
        &entity.version,
        &entity.begin_timestamp,
        &entity.end_timestamp,
        record_type,
        tx,
    )
    .await?;

    Ok(())
}

#[instrument(skip_all)]
async fn process_record_type(
    db_client: &PostgresClient,
    record_type: &RecordType,
    retention_period: &i64,
    storage_settings: &ObjectStorageSettings,
) -> Result<(), ScouterError> {
    info!("Archiving data for record type: {:?}", record_type);
    let df = ParquetDataFrame::new(storage_settings, record_type)?;

    // get the entities for archival
    let entities = get_entities_to_archive(db_client, record_type, retention_period).await?;

    // exit if no entities
    if entities.is_empty() {
        info!("No entities found for record type: {:?}", record_type);
        return Ok(());
    }

    // iterate over the entities and archive the data
    for entity in entities {
        // hold transaction here
        let mut tx = db_client
            .pool
            .begin()
            .await
            .map_err(|e| ScouterError::Error(e.to_string()))?;

        // get data for space/name/version
        let records = get_data_to_archive(&mut tx, record_type, &entity).await?;

        // get created at as YYYY-MM-DD string
        let begin_time = entity.begin_timestamp.format("%Y-%m-%d").to_string();
        // add random number to the created at string
        let random_hex: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(3)
            .map(char::from)
            .collect();

        let filename = format!("parquet-{}", random_hex);

        let rpath = format!(
            "{}/{}/{}/{}/{}/{}.parquet",
            entity.space, entity.name, entity.version, record_type, begin_time, filename
        );

        df.write_parquet(&rpath, records).await?;

        // update the entity in the database
        update_entities_to_archived(&mut tx, record_type, &entity).await?;

        tx.commit().await.map_err(|e| {
            error!("Error committing transaction: {}", e);
            ScouterError::Error(e.to_string())
        })?;
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
pub async fn archive_old_data(
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
