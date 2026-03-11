use crate::error::DataFrameError;
use arrow::array::AsArray;
use arrow::datatypes::UInt32Type;
use arrow_array::types::Float64Type;
use arrow_array::types::TimestampNanosecondType;
use arrow_array::Array;
use arrow_array::RecordBatch;
use arrow_array::StringViewArray;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::prelude::DataFrame;
use deltalake::logstore::{
    default_logstore, logstore_factories, LogStore, LogStoreFactory, ObjectStoreRef, StorageConfig,
};
use deltalake::DeltaResult;
use scouter_types::{BinnedMetric, BinnedMetricStats, BinnedMetrics};
use std::sync::Arc;
use tracing::{debug, error, instrument};
use url::Url;

/// Now that we have at least 2 metric types that calculate avg, lower_bound, and upper_bound as part of their stats,
/// it makes sense to implement a generic trait that we can use.
pub struct ParquetHelper {}

impl ParquetHelper {
    #[instrument(skip_all)]
    pub fn extract_feature_array(batch: &RecordBatch) -> Result<&StringViewArray, DataFrameError> {
        let feature_array = batch
            .column_by_name("feature")
            .ok_or_else(|| {
                error!("Missing 'feature' field in RecordBatch");
                DataFrameError::MissingFieldError("feature")
            })?
            .as_string_view_opt()
            .ok_or_else(|| {
                error!("Failed to downcast 'feature' field to StringViewArray");
                DataFrameError::DowncastError("StringViewArray")
            })?;
        Ok(feature_array)
    }

    #[instrument(skip_all)]
    pub fn extract_created_at(batch: &RecordBatch) -> Result<Vec<DateTime<Utc>>, DataFrameError> {
        let created_at_list = batch
            .column_by_name("created_at")
            .ok_or_else(|| {
                error!("Missing 'created_at' field in RecordBatch");
                DataFrameError::MissingFieldError("created_at")
            })?
            .as_list_opt::<i32>()
            .ok_or_else(|| {
                error!("Failed to downcast 'created_at' field to ListArray");
                DataFrameError::DowncastError("ListArray")
            })?;

        let created_at_array = created_at_list.value(0);
        Ok(created_at_array
            .as_primitive::<TimestampNanosecondType>()
            .iter()
            .filter_map(|ts| ts.map(|t| Utc.timestamp_nanos(t)))
            .collect())
    }
}
pub struct BinnedMetricsExtractor {}

impl BinnedMetricsExtractor {
    #[instrument(skip_all)]
    fn extract_stats(batch: &RecordBatch) -> Result<Vec<BinnedMetricStats>, DataFrameError> {
        let stats_list = batch
            .column_by_name("stats")
            .ok_or_else(|| {
                error!("Missing 'stats' field in RecordBatch");
                DataFrameError::MissingFieldError("stats")
            })?
            .as_list_opt::<i32>()
            .ok_or_else(|| {
                error!("Failed to downcast 'stats' field to ListArray");
                DataFrameError::DowncastError("ListArray")
            })?
            .value(0);

        let stats_structs = stats_list.as_struct_opt().ok_or_else(|| {
            error!("Failed to downcast 'stats' field to StructArray");
            DataFrameError::DowncastError("StructArray")
        })?;

        let avg_array = stats_structs
            .column_by_name("avg")
            .ok_or_else(|| DataFrameError::MissingFieldError("avg"))
            .inspect_err(|e| error!("Failed to get 'avg' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        let lower_bound_array = stats_structs
            .column_by_name("lower_bound")
            .ok_or_else(|| DataFrameError::MissingFieldError("lower_bound"))
            .inspect_err(|e| error!("Failed to get 'lower_bound' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        let upper_bound_array = stats_structs
            .column_by_name("upper_bound")
            .ok_or_else(|| DataFrameError::MissingFieldError("upper_bound"))
            .inspect_err(|e| error!("Failed to get 'upper_bound' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        Ok((0..stats_structs.len())
            .map(|i| BinnedMetricStats {
                avg: avg_array.value(i),
                lower_bound: lower_bound_array.value(i),
                upper_bound: upper_bound_array.value(i),
            })
            .collect())
    }

    #[instrument(skip_all)]
    fn process_metric_record_batch(batch: &RecordBatch) -> Result<BinnedMetric, DataFrameError> {
        debug!("Processing metric record batch");

        let metric_column = batch.column_by_name("metric").ok_or_else(|| {
            error!("Missing 'metric' field in RecordBatch");
            DataFrameError::MissingFieldError("metric")
        })?;

        // Handle both Dictionary and plain string types
        let metric_name = if let Some(dict_array) = metric_column.as_dictionary_opt::<UInt32Type>()
        {
            // Dictionary-encoded string (e.g., from GenAI task_id)
            let values = dict_array.values();
            let string_values = values.as_string_opt::<i32>().ok_or_else(|| {
                error!("Failed to downcast dictionary values to StringArray");
                DataFrameError::DowncastError("StringArray")
            })?;
            let key = dict_array.key(0).ok_or_else(|| {
                error!("Failed to get key from dictionary array");
                DataFrameError::MissingFieldError("dictionary key")
            })?;
            string_values.value(key).to_string()
        } else if let Some(string_view_array) = metric_column.as_string_view_opt() {
            // StringView type
            string_view_array.value(0).to_string()
        } else if let Some(string_array) = metric_column.as_string_opt::<i32>() {
            // Plain string type
            string_array.value(0).to_string()
        } else {
            error!("Failed to downcast 'metric' field to any supported string type");
            return Err(DataFrameError::DowncastError("String type"));
        };

        let created_at_list = ParquetHelper::extract_created_at(batch)?;
        let stats = Self::extract_stats(batch)?;

        Ok(BinnedMetric {
            metric: metric_name,
            created_at: created_at_list,
            stats,
        })
    }

    /// Convert a DataFrame to BinnedMetrics.
    ///
    /// # Arguments
    /// * `df` - The DataFrame to convert
    ///
    /// # Returns
    /// * `BinnedMetrics` - The converted BinnedMetrics
    #[instrument(skip_all)]
    pub async fn dataframe_to_binned_metrics(
        df: DataFrame,
    ) -> Result<BinnedMetrics, DataFrameError> {
        debug!("Converting DataFrame to binned metrics");

        let batches = df.collect().await?;

        let metrics: Vec<BinnedMetric> = batches
            .iter()
            .map(Self::process_metric_record_batch)
            .collect::<Result<Vec<_>, _>>()
            .inspect_err(|e| {
                error!("Failed to process metric record batch: {:?}", e);
            })?;

        Ok(BinnedMetrics::from_vec(metrics))
    }
}

pub(crate) struct PassthroughLogStoreFactory;

impl LogStoreFactory for PassthroughLogStoreFactory {
    fn with_options(
        &self,
        prefixed_store: ObjectStoreRef,
        root_store: ObjectStoreRef,
        location: &Url,
        options: &StorageConfig,
    ) -> DeltaResult<Arc<dyn LogStore>> {
        // For az:// URLs, object_store's ObjectStoreScheme::parse uses strip_bucket()
        // which assumes az://account/container/blob-path format. Scouter uses
        // az://container/blob-path (container in host, subpath in URL path).
        // strip_bucket() finds no second path segment → returns "" → delta-rs
        // applies no PrefixStore for Azure. Manually apply the correct prefix here.
        //
        // For gs://, s3://, s3a://, abfs://, abfss:// — delta-rs correctly derives
        // the subpath prefix from url.path() and applies PrefixStore via decorate_prefix.
        // Do not re-wrap those: use the already-prefixed `prefixed_store` as-is.
        let store = if location.scheme() == "az" {
            let subpath = location.path().trim_start_matches('/');
            if subpath.is_empty() {
                prefixed_store
            } else {
                let prefix = object_store::path::Path::from(subpath);
                Arc::new(object_store::prefix::PrefixStore::new(
                    root_store.clone(),
                    prefix,
                )) as ObjectStoreRef
            }
        } else {
            prefixed_store
        };
        Ok(default_logstore(store, root_store, location, options))
    }
}

pub(crate) fn register_cloud_logstore_factories() {
    let factories = logstore_factories();
    let factory = Arc::new(PassthroughLogStoreFactory) as Arc<dyn LogStoreFactory>;
    for scheme in ["gs", "s3", "s3a", "az", "abfs", "abfss"] {
        let key = Url::parse(&format!("{}://", scheme)).expect("scheme is a valid URL prefix");
        if !factories.contains_key(&key) {
            factories.insert(key, factory.clone());
        }
    }
}
