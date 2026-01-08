use crate::api::error::ServerError;
use chrono::{Duration, Utc};
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
/// Functionality for persisting data from postgres to long-term storage
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::traits::ArchiveSqlLogic;
use scouter_sql::{sql::schema::Entity, PostgresClient};
use scouter_types::{ArchiveRecord, DriftType, RecordType, ServerRecords};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use strum::IntoEnumIterator;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument};

pub struct DataArchiver {
    /// handler for background tasks
    pub workers: Vec<JoinHandle<()>>,
}

impl DataArchiver {
    /// Start a new data archiver worker
    pub async fn start_worker(
        id: usize,
        db_pool: Pool<Postgres>,
        config: Arc<ScouterServerConfig>,
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
                        // change this to days in subsequent pr. We need to test this in prod
                        // All unit and integration tests work, i'm just being a little paranoid and want to test in a live situation
                        Some(last_time) => now.signed_duration_since(last_time) >= Duration::hours(1),
                    };

                    if should_run {
                        match archive_old_data(&db_pool, &config).await {
                            Ok(_) => {
                                debug!("Archive completed successfully for worker {}", id);
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

/// Query database to get entities ready for archival
/// Returns a vector of entities uniquely identified by space/name/version
async fn get_entities_to_archive(
    db_pool: &Pool<Postgres>,
    record_type: &RecordType,
    retention_period: &i32,
) -> Result<Vec<Entity>, ServerError> {
    // get the data from the database
    let data = PostgresClient::get_entities_to_archive(db_pool, record_type, retention_period)
        .await
        .map_err(|e| {
            error!("Error getting entities to archive: {}", e);
            ServerError::GetEntitiesToArchiveError(e)
        })?;

    Ok(data)
}

/// Get data records for a given entity
async fn get_data_to_archive(
    db_pool: &Pool<Postgres>,
    record_type: &RecordType,
    entity: &Entity,
) -> Result<ServerRecords, ServerError> {
    // get the data from the database
    let data = PostgresClient::get_data_to_archive(
        &entity.entity_id,
        &entity.begin_timestamp,
        &entity.end_timestamp,
        record_type,
        db_pool,
    )
    .await
    .map_err(|e| {
        error!("Error getting data to archive: {}", e);
        ServerError::GetDataToArchiveError(e)
    })?;

    Ok(data)
}

/// Update the entity to archived in the database
/// Note - this doesn't delete the data from the database. It just marks it as archived
/// Deletion occurs via pg-cron
async fn update_entities_to_archived(
    db_pool: &Pool<Postgres>,
    record_type: &RecordType,
    entity: &Entity,
) -> Result<(), ServerError> {
    // get the data from the database
    PostgresClient::update_data_to_archived(
        &entity.entity_id,
        &entity.begin_timestamp,
        &entity.end_timestamp,
        record_type,
        db_pool,
    )
    .await
    .map_err(|e| {
        error!("Error updating data to archived: {}", e);
        ServerError::UpdateDataToArchivedError(e)
    })?;

    Ok(())
}

#[instrument(skip_all)]
async fn process_record_type(
    db_pool: &Pool<Postgres>,
    record_type: &RecordType,
    config: &Arc<ScouterServerConfig>,
) -> Result<bool, ServerError> {
    let df = ParquetDataFrame::new(&config.storage_settings, record_type)?;

    // get the entities for archival
    let entities = get_entities_to_archive(
        db_pool,
        record_type,
        &config.database_settings.retention_period,
    )
    .await?;

    // exit if no entities
    if entities.is_empty() {
        debug!("No entities found for record type: {:?}", record_type);
        return Ok(false);
    }

    // iterate over the entities and archive the data
    for entity in entities {
        // get data for space/name/version
        let records = get_data_to_archive(db_pool, record_type, &entity).await?;
        let rpath = entity.get_write_path(record_type);

        df.write_parquet(&rpath, records).await?;

        // update the entity in the database
        update_entities_to_archived(db_pool, record_type, &entity).await?;
    }

    debug!("Archiving data for record type: {:?} complete", record_type);

    Ok(true)
}

/// Parent function used to archive old data
///
/// # Arguments
/// * `db_client` - The database client to use for the archival
/// * `storage_settings` - The storage settings to use for the archival
/// * `retention_period` - The retention period to use for the archival
///
/// # Returns
/// * `Result<(), ServerError>` - The result of the archival
#[instrument(skip_all)]
pub async fn archive_old_data(
    db_pool: &Pool<Postgres>,
    config: &Arc<ScouterServerConfig>,
) -> Result<ArchiveRecord, ServerError> {
    // get old records
    debug!("Archiving old data");

    // TODO(Steven): Make this an audit event in the future
    let mut record = ArchiveRecord::default();

    for drift_type in DriftType::iter() {
        match drift_type {
            DriftType::Psi => {
                // get the data from the database
                record.psi = process_record_type(db_pool, &RecordType::Psi, config).await?;
            }
            DriftType::Spc => {
                // get the data from the database
                record.spc = process_record_type(db_pool, &RecordType::Spc, config).await?;
            }
            DriftType::Custom => {
                // get the data from the database
                record.custom = process_record_type(db_pool, &RecordType::Custom, config).await?;
            }
            DriftType::GenAI => {
                // process GenAI drift and metric records
                record.genai_task =
                    process_record_type(db_pool, &RecordType::GenAITask, config).await?;
                record.genai_event =
                    process_record_type(db_pool, &RecordType::GenAIEval, config).await?;
                record.genai_workflow =
                    process_record_type(db_pool, &RecordType::GenAIWorkflow, config).await?;
            }
        }
    }

    Ok(record)
}
