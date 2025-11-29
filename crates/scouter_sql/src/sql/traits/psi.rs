use crate::sql::query::Queries;
use crate::sql::schema::FeatureBinProportionResultWrapper;
use crate::sql::schema::FeatureDistributionWrapper;
use crate::sql::traits::entity;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scouter_dataframe::parquet::{dataframe_to_psi_drift_features, ParquetDataFrame};

use crate::sql::error::SqlError;
use itertools::multiunzip;
use scouter_settings::ObjectStorageSettings;
use scouter_types::psi::FeatureDistributions;
use scouter_types::{
    psi::FeatureBinProportionResult, DriftRequest, PsiRecord, RecordType, ServiceInfo,
};
use sqlx::{postgres::PgQueryResult, Pool, Postgres};
use std::collections::BTreeMap;
use tracing::{debug, instrument};

#[async_trait]
pub trait PsiSqlLogic {
    /// Inserts multiple PSI bin counts into the database in a batch.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `records` - The PSI server records to insert
    ///
    /// # Returns
    /// * A result containing the query result or an error
    async fn insert_bin_counts_batch(
        pool: &Pool<Postgres>,
        records: &[PsiRecord],
        entity_id: i32,
    ) -> Result<PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let query = Queries::InsertBinCountsBatch.get_query();

        let (created_ats, entity_ids, features, bin_ids, bin_counts): (
            Vec<DateTime<Utc>>,
            Vec<i32>,
            Vec<&str>,
            Vec<i64>,
            Vec<i64>,
        ) = multiunzip(records.iter().map(|r| {
            (
                r.created_at,
                entity_id,
                r.feature.as_str(),
                r.bin_id as i64,
                r.bin_count as i64,
            )
        }));

        sqlx::query(&query.sql)
            .bind(created_ats)
            .bind(entity_ids)
            .bind(features)
            .bind(bin_ids)
            .bind(bin_counts)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
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
            .map_err(SqlError::SqlxError)?
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

        Ok(dataframe_to_psi_drift_features(archived_df).await?)
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

    async fn get_feature_distributions(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        features_to_monitor: &[String],
    ) -> Result<FeatureDistributions, SqlError> {
        let query = Queries::GetFeatureBinProportions.get_query();

        let feature_distributions: Vec<FeatureDistributionWrapper> = sqlx::query_as(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(limit_datetime)
            .bind(features_to_monitor)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let distributions = feature_distributions
            .into_iter()
            .map(|wrapper| (wrapper.0, wrapper.1))
            .collect();

        Ok(FeatureDistributions { distributions })
    }
}
