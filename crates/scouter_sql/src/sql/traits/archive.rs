use crate::sql::query::Queries;
use crate::sql::schema::Entity;

use crate::sql::utils::pg_rows_to_server_records;
use chrono::{DateTime, Utc};

use scouter_error::SqlError;
use scouter_types::{RecordType, ServerRecords};

use sqlx::{Pool, Postgres, Transaction};

use std::result::Result::Ok;

use async_trait::async_trait;

#[async_trait]
pub trait ArchiveSqlLogic {
    /// Function to get entities for archival
    ///
    /// # Arguments
    /// * `record_type` - The type of record to get entities for
    /// * `retention_period` - The retention period to get entities for
    ///
    async fn get_entities_to_archive(
        pool: &Pool<Postgres>,
        record_type: &RecordType,
        retention_period: &i32,
    ) -> Result<Vec<Entity>, SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::GetSpcEntities.get_query(),
            RecordType::Psi => Queries::GetBinCountEntities.get_query(),
            RecordType::Custom => Queries::GetCustomEntities.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };

        let entities: Vec<Entity> = sqlx::query_as(&query.sql)
            .bind(retention_period)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_get_entities_error)?;

        Ok(entities)
    }

    /// Function to get data for archival
    ///
    /// # Arguments
    /// * `record_type` - The type of record to get data for
    /// * `days` - The number of days to get data for
    ///
    /// # Returns
    /// * `Result<ServerRecords, SqlError>` - Result of the query
    ///
    /// # Errors
    /// * `SqlError` - If the query fails
    async fn get_data_to_archive(
        space: &str,
        name: &str,
        version: &str,
        begin_timestamp: &DateTime<Utc>,
        end_timestamp: &DateTime<Utc>,
        record_type: &RecordType,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<ServerRecords, SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::GetSpcDataForArchive.get_query(),
            RecordType::Psi => Queries::GetBinCountDataForArchive.get_query(),
            RecordType::Custom => Queries::GetCustomDataForArchive.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };
        let rows = sqlx::query(&query.sql)
            .bind(begin_timestamp)
            .bind(end_timestamp)
            .bind(space)
            .bind(name)
            .bind(version)
            .fetch_all(&mut **tx)
            .await
            .map_err(SqlError::traced_get_entity_data_error)?;

        // need to convert the rows to server records (storage dataframe expects this)
        pg_rows_to_server_records(&rows, record_type)
    }

    async fn update_data_to_archived(
        space: &str,
        name: &str,
        version: &str,
        begin_timestamp: &DateTime<Utc>,
        end_timestamp: &DateTime<Utc>,
        record_type: &RecordType,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::UpdateSpcEntities.get_query(),
            RecordType::Psi => Queries::UpdateBinCountEntities.get_query(),
            RecordType::Custom => Queries::UpdateCustomEntities.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };
        sqlx::query(&query.sql)
            .bind(begin_timestamp)
            .bind(end_timestamp)
            .bind(space)
            .bind(name)
            .bind(version)
            .execute(&mut **tx)
            .await
            .map_err(SqlError::traced_get_entity_data_error)?;

        Ok(())
    }
}
