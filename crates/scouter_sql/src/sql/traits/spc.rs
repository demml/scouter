use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::schema::SpcFeatureResult;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_dataframe::parquet::{dataframe_to_spc_drift_features, ParquetDataFrame};
use scouter_settings::ObjectStorageSettings;
use scouter_types::{
    spc::{SpcDriftFeature, SpcDriftFeatures},
    DriftRequest, RecordType, ServiceInfo, SpcRecord,
};
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::BTreeMap;
use tracing::{debug, instrument};

#[async_trait]
pub trait SpcSqlLogic {
    /// Inserts a batch of SPC drift records into the database
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `records` - The SPC drift records to insert
    /// # Returns
    /// * A result containing the query result or an error
    async fn insert_spc_drift_records_batch(
        pool: &Pool<Postgres>,
        records: &[SpcRecord],
        entity_id: i32,
    ) -> Result<PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let query = Queries::InsertSpcDriftRecordBatch.get_query();

        let (created_ats, entity_ids, features, values): (
            Vec<DateTime<Utc>>,
            Vec<i32>,
            Vec<&str>,
            Vec<f64>,
        ) = multiunzip(
            records
                .iter()
                .map(|r| (r.created_at, entity_id, r.feature.as_str(), r.value)),
        );

        sqlx::query(&query.sql)
            .bind(created_ats)
            .bind(entity_ids)
            .bind(features)
            .bind(values)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    // Queries the database for all features under a service
    // Private method that'll be used to run drift retrieval in parallel
    async fn get_spc_features(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
    ) -> Result<Vec<String>, SqlError> {
        let query = Queries::GetSpcFeatures.get_query();

        Ok(sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .fetch_all(pool)
            .await
            .map(|result| {
                result
                    .iter()
                    .map(|row| row.get("feature"))
                    .collect::<Vec<String>>()
            })?)
    }

    /// Get SPC drift records
    ///
    /// # Arguments
    ///
    /// * `service_info` - The service to get drift records for
    /// * `limit_datetime` - The limit datetime to get drift records for
    /// * `features_to_monitor` - The features to monitor
    async fn get_spc_drift_records(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        features_to_monitor: &[String],
    ) -> Result<SpcDriftFeatures, SqlError> {
        let mut features = Self::get_spc_features(pool, service_info).await?;

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
            .await?;

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

    /// Queries the database for SPC drift records based on a time window
    /// and aggregation.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The drift request parameters
    ///
    /// # Returns
    /// * SpcDriftFeatures
    async fn get_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        minutes: i32,
    ) -> Result<SpcDriftFeatures, SqlError> {
        let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;

        let query = Queries::GetBinnedSpcFeatureValues.get_query();

        let records: Vec<SpcFeatureResult> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .fetch_all(pool)
            .await?;

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

    /// Helper for merging current and archived binned spc drift records.
    fn merge_feature_results(
        results: SpcDriftFeatures,
        map: &mut SpcDriftFeatures,
    ) -> Result<(), SqlError> {
        for (feature_name, feature) in results.features {
            let feature_clone = feature.clone();
            map.features
                .entry(feature_name)
                .and_modify(|existing| {
                    existing.created_at.extend(feature_clone.created_at);
                    existing.values.extend(feature_clone.values);
                })
                .or_insert(feature);
        }

        Ok(())
    }

    /// DataFusion implementation for getting spc drift records from archived data.
    ///
    /// # Arguments
    /// * `params` - The drift request parameters
    /// * `begin` - The start time of the time window
    /// * `end` - The end time of the time window
    /// * `minutes` - The number of minutes to bin the data
    /// * `storage_settings` - The object storage settings
    ///
    /// # Returns
    /// * A vector of drift records
    async fn get_archived_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<SpcDriftFeatures, SqlError> {
        let path = format!("{}/{}/{}/spc", params.space, params.name, params.version);
        let bin = minutes as f64 / params.max_data_points as f64;

        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::Spc)?
            .get_binned_metrics(
                &path,
                &bin,
                &begin,
                &end,
                &params.space,
                &params.name,
                &params.version,
            )
            .await?;

        Ok(dataframe_to_spc_drift_features(archived_df).await?)
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
    #[instrument(skip_all)]
    async fn get_binned_spc_drift_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<SpcDriftFeatures, SqlError> {
        debug!("Getting binned SPC drift records for {:?}", params);

        if !params.has_custom_interval() {
            debug!("No custom interval provided, using default");
            let minutes = params.time_interval.to_minutes();
            return Self::get_records(pool, params, minutes).await;
        }

        debug!("Custom interval provided, using custom interval");
        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.start, interval.end, retention_period)?;
        let mut spc_feature_map = SpcDriftFeatures::default();

        // get data from postgres
        if let Some(minutes) = timestamps.current_minutes {
            let current_results = Self::get_records(pool, params, minutes).await?;
            Self::merge_feature_results(current_results, &mut spc_feature_map)?;
        }

        // get archived data
        if let Some((archive_begin, archive_end)) = timestamps.archived_range {
            if let Some(archived_minutes) = timestamps.archived_minutes {
                let archived_results = Self::get_archived_records(
                    params,
                    archive_begin,
                    archive_end,
                    archived_minutes,
                    storage_settings,
                )
                .await?;

                Self::merge_feature_results(archived_results, &mut spc_feature_map)?;
            }
        }

        Ok(spc_feature_map)
    }
}
