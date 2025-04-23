use crate::sql::query::Queries;
use crate::sql::schema::FeatureBinProportionResultWrapper;
use crate::sql::schema::FeatureBinProportionWrapper;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scouter_contracts::{DriftRequest, ServiceInfo};
use scouter_dataframe::parquet::{dataframe_to_psi_drift_features, ParquetDataFrame};

use scouter_error::SqlError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{
    psi::{FeatureBinProportionResult, FeatureBinProportions},
    PsiServerRecord, RecordType,
};
use sqlx::{postgres::PgQueryResult, Pool, Postgres};
use std::collections::BTreeMap;
use tracing::{debug, instrument};

#[async_trait]
pub trait PsiSqlLogic {
    /// Inserts a PSI bin count into the database.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `record` - The PSI server record to insert
    ///
    /// # Returns
    /// * A result containing the query result or an error
    async fn insert_bin_counts(
        pool: &Pool<Postgres>,
        record: &PsiServerRecord,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertBinCounts.get_query();

        sqlx::query(&query.sql)
            .bind(record.created_at)
            .bind(&record.name)
            .bind(&record.space)
            .bind(&record.version)
            .bind(&record.feature)
            .bind(record.bin_id as i64)
            .bind(record.bin_count as i64)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Queries the database for PSI drift records based on a time window
    /// and aggregation.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The drift request parameters
    ///
    /// # Returns
    /// * A vector of drift records
    async fn get_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        minutes: i32,
    ) -> Result<Vec<FeatureBinProportionResult>, SqlError> {
        let bin = minutes as f64 / params.max_data_points as f64;
        let query = Queries::GetBinnedPsiFeatureBins.get_query();

        let binned: Vec<FeatureBinProportionResult> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?
            .into_iter()
            .map(|wrapper: FeatureBinProportionResultWrapper| wrapper.0)
            .collect();

        Ok(binned)
    }

    /// DataFusion implementation for getting PSI drift records from archived data.
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
    #[instrument(skip_all)]
    async fn get_archived_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Vec<FeatureBinProportionResult>, SqlError> {
        let path = format!("{}/{}/{}/psi", params.space, params.name, params.version);
        let bin = minutes as f64 / params.max_data_points as f64;

        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::Psi)?
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

        dataframe_to_psi_drift_features(archived_df)
            .await
            .map_err(SqlError::traced_failed_to_convert_dataframe_error)
    }

    /// Helper for merging current and archived binned PSI drift records.
    fn merge_feature_results(
        results: Vec<FeatureBinProportionResult>,
        feature_map: &mut BTreeMap<String, FeatureBinProportionResult>,
    ) -> Result<(), SqlError> {
        for result in results {
            feature_map
                .entry(result.feature.clone())
                .and_modify(|existing| {
                    existing.created_at.extend(result.created_at.iter());
                    existing
                        .bin_proportions
                        .extend(result.bin_proportions.iter().cloned());

                    for (k, v) in result.overall_proportions.iter() {
                        existing
                            .overall_proportions
                            .entry(*k)
                            .and_modify(|existing_value| {
                                *existing_value = (*existing_value + *v) / 2.0;
                            })
                            .or_insert(*v);
                    }
                })
                .or_insert(result);
        }

        Ok(())
    }

    // Queries the database for drift records based on a time window and aggregation.
    // Based on the time window provided, a query or queries will be run against the short-term and
    // archived data.
    //
    // # Arguments
    //
    // * `name` - The name of the service to query drift records for
    // * `params` - The drift request parameters
    // # Returns
    //
    // * A vector of drift records
    #[instrument(skip_all)]
    async fn get_binned_psi_drift_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Vec<FeatureBinProportionResult>, SqlError> {
        debug!("Getting binned PSI drift records for {:?}", params);
        if !params.has_custom_interval() {
            debug!("No custom interval provided, using default");
            let minutes = params.time_interval.to_minutes();
            return Self::get_records(pool, params, minutes).await;
        }

        debug!("Custom interval provided, using custom interval");
        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.start, interval.end, retention_period)?;
        let mut feature_map = BTreeMap::new();

        // Get current records if available
        if let Some(minutes) = timestamps.current_minutes {
            let current_results = Self::get_records(pool, params, minutes).await?;
            Self::merge_feature_results(current_results, &mut feature_map)?;
        }

        // Get archived records if available
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

                Self::merge_feature_results(archived_results, &mut feature_map)?;
            }
        }
        Ok(feature_map.into_values().collect())
    }

    async fn get_feature_bin_proportions(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        features_to_monitor: &[String],
    ) -> Result<FeatureBinProportions, SqlError> {
        let query = Queries::GetFeatureBinProportions.get_query();

        let binned: Vec<FeatureBinProportionWrapper> = sqlx::query_as(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(limit_datetime)
            .bind(features_to_monitor)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_get_bin_proportions_error)?;

        let binned: FeatureBinProportions = binned.into_iter().map(|wrapper| wrapper.0).collect();

        Ok(binned)
    }
}
