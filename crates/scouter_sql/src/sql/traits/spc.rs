use crate::sql::query::Queries;
use crate::sql::schema::SpcFeatureResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scouter_contracts::{DriftRequest, ServiceInfo};
use scouter_error::SqlError;
use scouter_types::{
    spc::{SpcDriftFeature, SpcDriftFeatures},
    SpcServerRecord,
};
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::BTreeMap;
#[async_trait]
pub trait SpcSqlLogic {
    /// Inserts a drift record into the database
    ///
    /// # Arguments
    ///
    /// * `record` - A drift record to insert into the database
    /// * `table_name` - The name of the table to insert the record into
    ///
    async fn insert_spc_drift_record(
        &self,
        pool: &Pool<Postgres>,
        record: &SpcServerRecord,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftRecord.get_query();

        sqlx::query(&query.sql)
            .bind(record.created_at)
            .bind(&record.name)
            .bind(&record.space)
            .bind(&record.version)
            .bind(&record.feature)
            .bind(record.value)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    // Queries the database for all features under a service
    // Private method that'll be used to run drift retrieval in parallel
    async fn get_spc_features(
        &self,
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
    ) -> Result<Vec<String>, SqlError> {
        let query = Queries::GetSpcFeatures.get_query();

        sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_get_features_error)
            .map(|result| {
                result
                    .iter()
                    .map(|row| row.get("feature"))
                    .collect::<Vec<String>>()
            })
    }

    /// Get SPC drift records
    ///
    /// # Arguments
    ///
    /// * `service_info` - The service to get drift records for
    /// * `limit_datetime` - The limit datetime to get drift records for
    /// * `features_to_monitor` - The features to monitor
    async fn get_spc_drift_records(
        &self,
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        features_to_monitor: &[String],
    ) -> Result<SpcDriftFeatures, SqlError> {
        let mut features = self.get_spc_features(pool, service_info).await?;

        if !features_to_monitor.is_empty() {
            features.retain(|feature| features_to_monitor.contains(feature));
        }

        let query = Queries::GetSpcFeatureValues.get_query();

        let records: Vec<SpcFeatureResult> = sqlx::query_as(&query.sql)
            .bind(limit_datetime)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(features)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        let feature_drift = records
            .into_iter()
            .map(|record| {
                let feature = SpcDriftFeature {
                    created_at: record.created_at,
                    values: record.values,
                };
                (record.feature.clone(), feature)
            })
            .collect::<BTreeMap<String, SpcDriftFeature>>();

        Ok(SpcDriftFeatures {
            features: feature_drift,
        })
    }

    // Queries the database for drift records based on a time window and aggregation
    //
    // # Arguments
    //
    // * `name` - The name of the service to query drift records for
    // * `space` - The name of the space to query drift records for
    // * `feature` - The name of the feature to query drift records for
    // * `aggregation` - The aggregation to use for the query
    // * `time_interval` - The time window to query drift records for
    //
    // # Returns
    //
    // * A vector of drift records
    async fn get_binned_spc_drift_records(
        &self,
        pool: &Pool<Postgres>,
        params: &DriftRequest,
    ) -> Result<SpcDriftFeatures, SqlError> {
        let minutes = params.time_interval.to_minutes();
        let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;

        let query = Queries::GetBinnedSpcFeatureValues.get_query();

        let records: Vec<SpcFeatureResult> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        let feature_drift = records
            .into_iter()
            .map(|record| {
                let feature = SpcDriftFeature {
                    created_at: record.created_at,
                    values: record.values,
                };
                (record.feature.clone(), feature)
            })
            .collect::<BTreeMap<String, SpcDriftFeature>>();

        Ok(SpcDriftFeatures {
            features: feature_drift,
        })
    }
}
