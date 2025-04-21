use crate::sql::query::Queries;
use crate::sql::schema::{
    AlertWrapper, BinnedCustomMetricWrapper, Entity, FeatureBinProportionResultWrapper,
    FeatureBinProportionWrapper, ObservabilityResult, SpcFeatureResult, TaskRequest,
    UpdateAlertResult, User,
};
use crate::sql::utils::pg_rows_to_server_records;
use chrono::{DateTime, Utc};
use cron::Schedule;
use scouter_contracts::{
    DriftAlertRequest, DriftRequest, GetProfileRequest, ObservabilityMetricRequest,
    ProfileStatusRequest, ServiceInfo, UpdateAlertStatus,
};
use scouter_dataframe::parquet::{
    dataframe_to_custom_drift_metrics, dataframe_to_psi_drift_features,
    dataframe_to_spc_drift_features, ParquetDataFrame,
};
use scouter_error::{ScouterError, SqlError, UtilError};
use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
use scouter_types::psi::FeatureBinProportionResult;
use scouter_types::DriftType;
use scouter_types::{
    alert::Alert,
    custom::BinnedCustomMetrics,
    psi::FeatureBinProportions,
    spc::{SpcDriftFeature, SpcDriftFeatures},
    CustomMetricServerRecord, DriftProfile, ObservabilityMetrics, PsiServerRecord, RecordType,
    ServerRecords, SpcServerRecord, TimeInterval, ToDriftRecords,
};
use serde_json::Value;
use sqlx::{
    postgres::{PgPoolOptions, PgQueryResult},
    Pool, Postgres, Row, Transaction,
};
use std::collections::{BTreeMap, HashMap};
use std::result::Result::Ok;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use super::utils::split_custom_interval;

// TODO: Explore refactoring and breaking this out into multiple client types (i.e., spc, psi, etc.)
// Postgres client is one of the lowest-level abstractions so it may not be worth it, as it could make server logic annoying. Worth exploring though.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PostgresClient {
    pub pool: Pool<Postgres>,
    pub retention_period: i64,
    pub storage_settings: ObjectStorageSettings,
}

impl PostgresClient {
    /// Create a new PostgresClient
    ///
    /// # Arguments
    ///
    /// * `pool` - An optional database pool
    ///
    /// # Returns
    ///
    /// * `Result<Self, SqlError>` - Result of the database pool
    pub async fn new(
        pool: Option<Pool<Postgres>>,
        database_settings: &DatabaseSettings,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Arc<Self>, SqlError> {
        let pool = pool.unwrap_or(
            Self::create_db_pool(database_settings)
                .await
                .map_err(SqlError::traced_connection_error)?,
        );
        let retention_period = database_settings.retention_period;

        let client = Self {
            pool,
            retention_period,
            storage_settings: storage_settings.clone(),
        };

        // run migrations
        client.run_migrations().await?;

        Ok(Arc::new(client))
    }

    /// Setup the application with the given database pool.
    ///
    /// # Returns
    ///
    /// * `Result<Pool<Postgres>, anyhow::Error>` - Result of the database pool
    #[instrument(skip(database_settings))]
    pub async fn create_db_pool(
        database_settings: &DatabaseSettings,
    ) -> Result<Pool<Postgres>, SqlError> {
        let pool = match PgPoolOptions::new()
            .max_connections(database_settings.max_connections)
            .connect(&database_settings.connection_uri)
            .await
        {
            Ok(pool) => {
                info!("✅ Successfully connected to database");
                pool
            }
            Err(err) => {
                error!("🚨 Failed to connect to database {:?}", err);
                std::process::exit(1);
            }
        };

        Ok(pool)
    }

    async fn run_migrations(&self) -> Result<(), SqlError> {
        info!("Running migrations");
        sqlx::migrate!("src/migrations")
            .run(&self.pool)
            .await
            .map_err(|e| SqlError::MigrationError(format!("{}", e)))?;

        debug!("Migrations complete");

        Ok(())
    }

    /// Inserts a drift alert into the database
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the service to insert the alert for
    /// * `space` - The name of the space to insert the alert for
    /// * `version` - The version of the service to insert the alert for
    /// * `alert` - The alert to insert into the database
    ///
    pub async fn insert_drift_alert(
        &self,
        service_info: &ServiceInfo,
        feature: &str,
        alert: &BTreeMap<String, String>,
        drift_type: &DriftType,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftAlert.get_query();

        let query_result: std::result::Result<PgQueryResult, SqlError> = sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(feature)
            .bind(serde_json::to_value(alert).unwrap())
            .bind(drift_type.to_string())
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        match query_result {
            Ok(result) => Ok(result),
            Err(e) => Err(e),
        }
    }

    /// Get drift alerts from the database
    ///
    /// # Arguments
    ///
    /// * `params` - The drift alert request parameters
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Alert>, SqlError>` - Result of the query
    pub async fn get_drift_alerts(
        &self,
        params: &DriftAlertRequest,
    ) -> Result<Vec<Alert>, SqlError> {
        let query = Queries::GetDriftAlerts.get_query().sql;

        // check if active (status can be 'active' or  'acknowledged')
        let query = if params.active.unwrap_or(false) {
            format!("{} AND active = true", query)
        } else {
            query
        };

        let query = format!("{} ORDER BY created_at DESC", query);

        let query = if let Some(limit) = params.limit {
            format!("{} LIMIT {}", query, limit)
        } else {
            query
        };

        // convert limit timestamp to string if it exists, leave as None if not

        let result: Result<Vec<AlertWrapper>, SqlError> = sqlx::query_as(&query)
            .bind(&params.version)
            .bind(&params.name)
            .bind(&params.space)
            .bind(params.limit_datetime)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        result.map(|result| result.into_iter().map(|wrapper| wrapper.0).collect())
    }

    pub async fn update_drift_alert_status(
        &self,
        params: &UpdateAlertStatus,
    ) -> Result<UpdateAlertResult, SqlError> {
        let query = Queries::UpdateAlertStatus.get_query();

        let result: Result<UpdateAlertResult, SqlError> = sqlx::query_as(&query.sql)
            .bind(params.id)
            .bind(params.active)
            .fetch_one(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        match result {
            Ok(result) => Ok(result),
            Err(e) => Err(SqlError::traced_query_error(e.to_string())),
        }
    }

    /// Inserts a drift record into the database
    ///
    /// # Arguments
    ///
    /// * `record` - A drift record to insert into the database
    /// * `table_name` - The name of the table to insert the record into
    ///
    pub async fn insert_spc_drift_record(
        &self,
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
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    pub async fn insert_bin_counts(
        &self,
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
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    // Inserts a drift record into the database
    //
    // # Arguments
    //
    // * `record` - A drift record to insert into the database
    // * `table_name` - The name of the table to insert the record into
    //
    pub async fn insert_observability_record(
        &self,
        record: &ObservabilityMetrics,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertObservabilityRecord.get_query();
        let route_metrics = serde_json::to_value(&record.route_metrics)
            .map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(&record.space)
            .bind(&record.name)
            .bind(&record.version)
            .bind(record.request_count)
            .bind(record.error_count)
            .bind(route_metrics)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Insert a drift profile into the database
    ///
    /// # Arguments
    ///
    /// * `drift_profile` - The drift profile to insert
    ///
    /// # Returns
    ///
    /// * `Result<PgQueryResult, SqlError>` - Result of the query
    pub async fn insert_drift_profile(
        &self,
        drift_profile: &DriftProfile,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftProfile.get_query();
        let base_args = drift_profile.get_base_args();

        let current_time = Utc::now();

        let schedule =
            Schedule::from_str(&base_args.schedule).map_err(UtilError::traced_parse_cron_error)?;

        let next_run = schedule
            .upcoming(Utc)
            .take(1)
            .next()
            .ok_or(SqlError::traced_get_next_run_error(&base_args.schedule))?;

        sqlx::query(&query.sql)
            .bind(base_args.name)
            .bind(base_args.space)
            .bind(base_args.version)
            .bind(base_args.scouter_version)
            .bind(drift_profile.to_value())
            .bind(base_args.drift_type.to_string())
            .bind(false)
            .bind(base_args.schedule)
            .bind(next_run)
            .bind(current_time)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Update a drift profile in the database
    ///
    /// # Arguments
    ///
    /// * `drift_profile` - The drift profile to update
    ///
    /// # Returns
    ///
    /// * `Result<PgQueryResult, SqlError>` - Result of the query
    pub async fn update_drift_profile(
        &self,
        drift_profile: &DriftProfile,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::UpdateDriftProfile.get_query();
        let base_args = drift_profile.get_base_args();

        sqlx::query(&query.sql)
            .bind(drift_profile.to_value())
            .bind(base_args.drift_type.to_string())
            .bind(base_args.name)
            .bind(base_args.space)
            .bind(base_args.version)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Get a drift profile from the database
    ///
    /// # Arguments
    ///
    /// * `request` - The request to get the profile for
    ///
    /// # Returns
    pub async fn get_drift_profile(
        &self,
        request: &GetProfileRequest,
    ) -> Result<Option<Value>, SqlError> {
        let query = Queries::GetDriftProfile.get_query();

        let result = sqlx::query(&query.sql)
            .bind(&request.name)
            .bind(&request.space)
            .bind(&request.version)
            .bind(request.drift_type.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        match result {
            Some(result) => {
                let profile: Value = result.get("profile");
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    pub async fn get_drift_profile_task(
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<TaskRequest>, SqlError> {
        let query = Queries::GetDriftTask.get_query();
        let result: Result<Option<TaskRequest>, sqlx::Error> = sqlx::query_as(&query.sql)
            .fetch_optional(&mut **transaction)
            .await;

        result.map_err(SqlError::traced_get_drift_task_error)
    }

    /// Update the drift profile run dates in the database
    ///
    /// # Arguments
    ///
    /// * `transaction` - The database transaction
    /// * `service_info` - The service info to update the run dates for
    /// * `schedule` - The schedule to update the run dates with
    ///
    /// # Returns
    ///
    /// * `Result<(), SqlError>` - Result of the query
    pub async fn update_drift_profile_run_dates(
        transaction: &mut Transaction<'_, Postgres>,
        service_info: &ServiceInfo,
        schedule: &str,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateDriftProfileRunDates.get_query();

        let schedule = Schedule::from_str(schedule).map_err(UtilError::traced_parse_cron_error)?;

        let next_run = schedule
            .upcoming(Utc)
            .take(1)
            .next()
            .ok_or(SqlError::traced_get_next_run_error(schedule))?;

        let query_result = sqlx::query(&query.sql)
            .bind(next_run)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .execute(&mut **transaction)
            .await;

        match query_result {
            Ok(_) => Ok(()),
            Err(e) => Err(SqlError::traced_update_drift_profile_error(e)),
        }
    }

    // Queries the database for all features under a service
    // Private method that'll be used to run drift retrieval in parallel
    pub async fn get_spc_features(
        &self,
        service_info: &ServiceInfo,
    ) -> Result<Vec<String>, SqlError> {
        let query = Queries::GetSpcFeatures.get_query();

        sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .fetch_all(&self.pool)
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
    pub async fn get_spc_drift_records(
        &self,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        features_to_monitor: &[String],
    ) -> Result<SpcDriftFeatures, SqlError> {
        let mut features = self.get_spc_features(service_info).await?;

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
            .fetch_all(&self.pool)
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

    pub async fn get_binned_observability_metrics(
        &self,
        params: &ObservabilityMetricRequest,
    ) -> Result<Vec<ObservabilityResult>, SqlError> {
        let query = Queries::GetBinnedObservabilityMetrics.get_query();

        let time_interval = TimeInterval::from_string(&params.time_interval).to_minutes();

        let bin = time_interval as f64 / params.max_data_points as f64;

        let observability_metrics: Result<Vec<ObservabilityResult>, sqlx::Error> =
            sqlx::query_as(&query.sql)
                .bind(bin)
                .bind(time_interval)
                .bind(&params.name)
                .bind(&params.space)
                .bind(&params.version)
                .fetch_all(&self.pool)
                .await;

        observability_metrics.map_err(SqlError::traced_query_error)
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
    pub async fn get_binned_spc_drift_records(
        &self,
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
            .fetch_all(&self.pool)
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
    // * `params` - The drift request parameters
    // # Returns
    //
    // * A vector of drift records
    pub async fn get_binned_psi_drift_records(
        &self,
        params: &DriftRequest,
    ) -> Result<Vec<FeatureBinProportionResult>, SqlError> {
        let mut results = Vec::new();
        if params.has_custom_interval() {
            let mut feature_map: BTreeMap<String, FeatureBinProportionResult> = BTreeMap::new();
            let interval = params.clone().custom_interval.unwrap();
            let timestamps =
                split_custom_interval(interval.start, interval.end, &self.retention_period)?;

            if let Some((archive_begin, archive_end)) = timestamps.archived_range {
                let path = format!("{}/{}/{}/psi", params.space, params.name, params.version);
                let archived_minutes = timestamps.archived_minutes.unwrap() as f64;
                let bin = archived_minutes / params.max_data_points as f64;

                let archived_df = ParquetDataFrame::new(&self.storage_settings, &RecordType::Psi)?
                    .get_binned_metrics(
                        &path,
                        &bin,
                        &archive_begin,
                        &archive_end,
                        &params.space,
                        &params.name,
                        &params.version,
                    )
                    .await?;

                let archived_results = dataframe_to_psi_drift_features(archived_df)
                    .await
                    .map_err(SqlError::traced_failed_to_convert_dataframe_error)?;

                for result in archived_results {
                    feature_map
                        .entry(result.feature.clone())
                        .and_modify(|existing| {
                            existing.created_at.extend(result.created_at);
                            existing.bin_proportions.extend(result.bin_proportions);
                            // Merge overall proportions
                            for (k, v) in result.overall_proportions {
                                existing.overall_proportions.insert(k, v);
                            }
                        })
                        .or_insert(result);
                }
            }

            if let Some(minutes) = timestamps.current_minutes {
                let query = Queries::GetBinnedPsiFeatureBins.get_query();
                let bin = minutes as f64 / params.max_data_points as f64;
                let current_results: Vec<FeatureBinProportionResult> = sqlx::query_as(&query.sql)
                    .bind(bin)
                    .bind(minutes)
                    .bind(&params.name)
                    .bind(&params.space)
                    .bind(&params.version)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(SqlError::traced_query_error)?
                    .into_iter()
                    .map(|wrapper: FeatureBinProportionResultWrapper| wrapper.0)
                    .collect();

                results.extend(current_results);
            }
        } else {
            // get features

            let minutes = params.time_interval.to_minutes();
            let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;
            let query = Queries::GetBinnedPsiFeatureBins.get_query();
            let binned: Vec<FeatureBinProportionResult> = sqlx::query_as(&query.sql)
                .bind(bin)
                .bind(minutes)
                .bind(&params.name)
                .bind(&params.space)
                .bind(&params.version)
                .fetch_all(&self.pool)
                .await
                .map_err(SqlError::traced_query_error)?
                .into_iter()
                .map(|wrapper: FeatureBinProportionResultWrapper| wrapper.0)
                .collect();

            results.extend(binned);
        }

        results.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(results)
    }

    // Queries the database for drift records based on a time window and aggregation
    //
    // # Arguments
    //
    // * `name` - The name of the service to query drift records for
    // * `params` - The drift request parameters
    // # Returns
    //
    // * A vector of drift records
    pub async fn get_binned_custom_drift_records(
        &self,
        params: &DriftRequest,
    ) -> Result<BinnedCustomMetrics, SqlError> {
        // get features

        let minutes = params.time_interval.to_minutes();
        let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;

        let query = Queries::GetBinnedCustomMetricValues.get_query();

        let records: Vec<BinnedCustomMetricWrapper> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(BinnedCustomMetrics::from_vec(
            records.into_iter().map(|wrapper| wrapper.0).collect(),
        ))
    }

    pub async fn update_drift_profile_status(
        &self,
        params: &ProfileStatusRequest,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateDriftProfileStatus.get_query();

        // convert drift_type to string or None
        let query_result = sqlx::query(&query.sql)
            .bind(params.active)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .bind(params.drift_type.as_ref().map(|t| t.to_string()))
            .execute(&self.pool)
            .await;

        match query_result {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to update drift profile status: {:?}", e);
                Err(SqlError::traced_update_drift_profile_error(e))
            }
        }
    }

    pub async fn get_feature_bin_proportions(
        &self,
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
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_get_bin_proportions_error)?;

        let binned: FeatureBinProportions = binned.into_iter().map(|wrapper| wrapper.0).collect();

        Ok(binned)
    }

    pub async fn get_custom_metric_values(
        &self,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        metrics: &[String],
    ) -> Result<HashMap<String, f64>, SqlError> {
        let query = Queries::GetCustomMetricValues.get_query();

        let records = sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(limit_datetime)
            .bind(metrics)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_get_custom_metrics_error)?;

        let metric_map = records
            .into_iter()
            .map(|row| {
                let metric = row.get("metric");
                let value = row.get("value");
                (metric, value)
            })
            .collect();

        Ok(metric_map)
    }

    pub async fn insert_custom_metric_value(
        &self,
        record: &CustomMetricServerRecord,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertCustomMetricValues.get_query();

        let query_result = sqlx::query(&query.sql)
            .bind(record.created_at)
            .bind(&record.name)
            .bind(&record.space)
            .bind(&record.version)
            .bind(&record.metric)
            .bind(record.value)
            .execute(&self.pool)
            .await;

        match query_result {
            Ok(result) => Ok(result),
            Err(e) => Err(SqlError::traced_insert_custom_metrics_error(e)),
        }
    }

    /// Function to get entities for archival
    ///
    /// # Arguments
    /// * `record_type` - The type of record to get entities for
    /// * `retention_period` - The retention period to get entities for
    ///
    pub async fn get_entities_to_archive(
        &self,
        record_type: &RecordType,
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
            .bind(&self.retention_period)
            .fetch_all(&self.pool)
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
    pub async fn get_data_to_archive(
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

    pub async fn update_data_to_archived(
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

    pub async fn insert_user(&self, user: &User) -> Result<(), SqlError> {
        let query = Queries::InsertUser.get_query();

        let group_permissions = serde_json::to_value(&user.group_permissions)
            .map_err(UtilError::traced_serialize_error)?;

        let permissions =
            serde_json::to_value(&user.permissions).map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&permissions)
            .bind(&group_permissions)
            .bind(&user.role)
            .bind(user.active)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }

    pub async fn get_user(&self, username: &str) -> Result<Option<User>, SqlError> {
        let query = Queries::GetUser.get_query();

        let user: Option<User> = sqlx::query_as(&query.sql)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(user)
    }

    pub async fn update_user(&self, user: &User) -> Result<(), SqlError> {
        let query = Queries::UpdateUser.get_query();

        let group_permissions = serde_json::to_value(&user.group_permissions)
            .map_err(UtilError::traced_serialize_error)?;

        let permissions =
            serde_json::to_value(&user.permissions).map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(user.active)
            .bind(&user.password_hash)
            .bind(&permissions)
            .bind(&group_permissions)
            .bind(&user.refresh_token)
            .bind(&user.username)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }

    pub async fn get_users(&self) -> Result<Vec<User>, SqlError> {
        let query = Queries::GetUsers.get_query();

        let users = sqlx::query_as::<_, User>(&query.sql)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(users)
    }

    pub async fn is_last_admin(&self, username: &str) -> Result<bool, SqlError> {
        // Count admins in the system
        let query = Queries::LastAdmin.get_query();

        let admins: Vec<String> = sqlx::query_scalar(&query.sql)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        // check if length is 1 and the username is the same
        if admins.len() > 1 {
            return Ok(false);
        }

        // no admins found
        if admins.is_empty() {
            return Ok(false);
        }

        // check if the username is the last admin
        Ok(admins.len() == 1 && admins[0] == username)
    }

    pub async fn delete_user(&self, username: &str) -> Result<(), SqlError> {
        let query = Queries::DeleteUser.get_query();

        sqlx::query(&query.sql)
            .bind(username)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(())
    }
}

pub enum MessageHandler {
    Postgres(Arc<PostgresClient>),
}

impl MessageHandler {
    #[instrument(skip(self, records), name = "Insert Server Records")]
    pub async fn insert_server_records(&self, records: &ServerRecords) -> Result<(), ScouterError> {
        match self {
            Self::Postgres(client) => {
                match records.record_type()? {
                    RecordType::Spc => {
                        debug!("SPC record count: {:?}", records.len());
                        let records = records.to_spc_drift_records()?;
                        for record in records.iter() {
                            let _ = client.insert_spc_drift_record(record).await.map_err(|e| {
                                error!("Failed to insert drift record: {:?}", e);
                            });
                        }
                    }
                    RecordType::Observability => {
                        debug!("Observability record count: {:?}", records.len());
                        let records = records.to_observability_drift_records()?;
                        for record in records.iter() {
                            let _ = client
                                .insert_observability_record(record)
                                .await
                                .map_err(|e| {
                                    error!("Failed to insert observability record: {:?}", e);
                                });
                        }
                    }
                    RecordType::Psi => {
                        debug!("PSI record count: {:?}", records.len());
                        let records = records.to_psi_drift_records()?;
                        for record in records.iter() {
                            let _ = client.insert_bin_counts(record).await.map_err(|e| {
                                error!("Failed to insert bin count record: {:?}", e);
                            });
                        }
                    }
                    RecordType::Custom => {
                        debug!("Custom record count: {:?}", records.len());
                        let records = records.to_custom_metric_drift_records()?;
                        for record in records.iter() {
                            let _ = client
                                .insert_custom_metric_value(record)
                                .await
                                .map_err(|e| {
                                    error!("Failed to insert bin count record: {:?}", e);
                                });
                        }
                    }
                };
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::Rng;
    use scouter_types::{spc::SpcDriftProfile, DriftType};

    const SPACE: &str = "space";
    const NAME: &str = "name";
    const VERSION: &str = "1.0.0";

    pub async fn cleanup(pool: &Pool<Postgres>) {
        sqlx::raw_sql(
            r#"
            DELETE
            FROM scouter.spc_drift;

            DELETE
            FROM scouter.observability_metric;

            DELETE
            FROM scouter.custom_metric;

            DELETE
            FROM scouter.drift_alert;

            DELETE
            FROM scouter.drift_profile;

            DELETE
            FROM scouter.observed_bin_count;

             DELETE
            FROM scouter.user;
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap();
    }

    pub async fn db_client() -> Arc<PostgresClient> {
        let client = PostgresClient::new(
            None,
            &DatabaseSettings::default(),
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();

        cleanup(&client.pool).await;

        client
    }

    #[tokio::test]
    async fn test_postgres() {
        let _client = db_client().await;
    }

    #[tokio::test]
    async fn test_postgres_drift_alert() {
        let client = db_client().await;

        let timestamp = Utc::now();

        for _ in 0..10 {
            let service_info = ServiceInfo {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
            };

            let alert = (0..10)
                .map(|i| (i.to_string(), i.to_string()))
                .collect::<BTreeMap<String, String>>();

            let result = client
                .insert_drift_alert(&service_info, "test", &alert, &DriftType::Spc)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // get alerts
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: None,
        };

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
        assert!(alerts.len() > 5);

        // get alerts limit 1
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: Some(1),
            limit_datetime: None,
        };

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
        assert_eq!(alerts.len(), 1);

        // get alerts limit timestamp
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: Some(timestamp),
        };

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
        assert!(alerts.len() > 5);
    }

    #[tokio::test]
    async fn test_postgres_spc_drift_record() {
        let client = db_client().await;

        let record = SpcServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            value: 1.0,
        };

        let result = client.insert_spc_drift_record(&record).await.unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_bin_count() {
        let client = db_client().await;

        let record = PsiServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            bin_id: 1,
            bin_count: 1,
        };

        let result = client.insert_bin_counts(&record).await.unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_observability_record() {
        let client = db_client().await;

        let record = ObservabilityMetrics::default();

        let result = client.insert_observability_record(&record).await.unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_crud_drift_profile() {
        let client = PostgresClient::new(
            None,
            &DatabaseSettings::default(),
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();
        cleanup(&client.pool).await;

        let mut spc_profile = SpcDriftProfile::default();

        let result = client
            .insert_drift_profile(&DriftProfile::Spc(spc_profile.clone()))
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);

        spc_profile.scouter_version = "test".to_string();

        let result = client
            .update_drift_profile(&DriftProfile::Spc(spc_profile.clone()))
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);

        let profile = client
            .get_drift_profile(&GetProfileRequest {
                name: spc_profile.config.name.clone(),
                space: spc_profile.config.space.clone(),
                version: spc_profile.config.version.clone(),
                drift_type: DriftType::Spc,
            })
            .await
            .unwrap();

        let deserialized = serde_json::from_value::<SpcDriftProfile>(profile.unwrap()).unwrap();

        assert_eq!(deserialized, spc_profile);

        client
            .update_drift_profile_status(&ProfileStatusRequest {
                name: spc_profile.config.name.clone(),
                space: spc_profile.config.space.clone(),
                version: spc_profile.config.version.clone(),
                active: false,
                drift_type: Some(DriftType::Spc),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_postgres_get_features() {
        let client = db_client().await;

        let timestamp = Utc::now();

        for _ in 0..10 {
            for j in 0..10 {
                let record = SpcServerRecord {
                    created_at: Utc::now(),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    feature: format!("test{}", j),
                    value: j as f64,
                };

                let result = client.insert_spc_drift_record(&record).await.unwrap();
                assert_eq!(result.rows_affected(), 1);
            }
        }

        let service_info = ServiceInfo {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
        };

        let features = client.get_spc_features(&service_info).await.unwrap();
        assert_eq!(features.len(), 10);

        let records = client
            .get_spc_drift_records(&service_info, &timestamp, &features)
            .await
            .unwrap();

        assert_eq!(records.features.len(), 10);

        let binned_records = client
            .get_binned_spc_drift_records(&DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::FiveMinutes,
                max_data_points: 10,
                drift_type: DriftType::Spc,
                custom_interval: None,
            })
            .await
            .unwrap();

        assert_eq!(binned_records.features.len(), 10);
    }

    #[tokio::test]
    async fn test_postgres_bin_proportions() {
        let client = db_client().await;

        let timestamp = Utc::now();

        for feature in 0..3 {
            for bin in 0..=5 {
                for _ in 0..=100 {
                    let record = PsiServerRecord {
                        created_at: Utc::now(),
                        space: SPACE.to_string(),
                        name: NAME.to_string(),
                        version: VERSION.to_string(),
                        feature: format!("feature{}", feature),
                        bin_id: bin,
                        bin_count: rand::rng().random_range(0..10),
                    };

                    client.insert_bin_counts(&record).await.unwrap();
                }
            }
        }

        let binned_records = client
            .get_feature_bin_proportions(
                &ServiceInfo {
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                },
                &timestamp,
                &["feature0".to_string()],
            )
            .await
            .unwrap();

        // assert binned_records.features["test"]["decile_1"] is around .5
        let bin_proportion = binned_records
            .features
            .get("feature0")
            .unwrap()
            .get(&1)
            .unwrap();

        assert!(*bin_proportion > 0.1 && *bin_proportion < 0.2);

        let binned_records = client
            .get_binned_psi_drift_records(&DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                drift_type: DriftType::Psi,
                custom_interval: None,
            })
            .await
            .unwrap();
        //
        assert_eq!(binned_records.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_cru_custom_metric() {
        let client = db_client().await;

        let timestamp = Utc::now();

        for i in 0..2 {
            for _ in 0..25 {
                let record = CustomMetricServerRecord {
                    created_at: Utc::now(),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    metric: format!("metric{}", i),
                    value: rand::rng().random_range(0..10) as f64,
                };

                let result = client.insert_custom_metric_value(&record).await.unwrap();
                assert_eq!(result.rows_affected(), 1);
            }
        }

        // insert random record to test has statistics funcs handle single record
        let record = CustomMetricServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            metric: "metric3".to_string(),
            value: rand::rng().random_range(0..10) as f64,
        };

        let result = client.insert_custom_metric_value(&record).await.unwrap();
        assert_eq!(result.rows_affected(), 1);

        let metrics = client
            .get_custom_metric_values(
                &ServiceInfo {
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                },
                &timestamp,
                &["metric1".to_string()],
            )
            .await
            .unwrap();

        assert_eq!(metrics.len(), 1);

        let binned_records = client
            .get_binned_custom_drift_records(&DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                drift_type: DriftType::Custom,
                custom_interval: None,
            })
            .await
            .unwrap();
        //
        assert_eq!(binned_records.metrics.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_user() {
        let client = db_client().await;

        // Create
        let user = User::new(
            "user".to_string(),
            "pass".to_string(),
            None,
            None,
            Some("admin".to_string()),
        );
        client.insert_user(&user).await.unwrap();

        // Read
        let mut user = client.get_user("user").await.unwrap().unwrap();
        assert_eq!(user.username, "user");

        // update user
        user.active = false;
        user.refresh_token = Some("token".to_string());

        // Update
        client.update_user(&user).await.unwrap();
        let user = client.get_user("user").await.unwrap().unwrap();
        assert!(!user.active);
        assert_eq!(user.refresh_token.unwrap(), "token");

        // get users
        let users = client.get_users().await.unwrap();
        assert_eq!(users.len(), 1);

        // get last admin
        let is_last_admin = client.is_last_admin("user").await.unwrap();
        assert!(is_last_admin);

        // delete
        client.delete_user("user").await.unwrap();
    }
}
