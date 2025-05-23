use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::types::BinnedTableName;
use crate::sql::helper::get_binned_custom_metric_values_query;
use crate::storage::ObjectStore;
use arrow::array::AsArray;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{Float64Array, StringArray, TimestampNanosecondArray};
use arrow_array::types::Float64Type;
use arrow_array::{ListArray, StringViewArray};
use arrow_array::{RecordBatch, StructArray};
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;

use scouter_types::{
    custom::{BinnedCustomMetric, BinnedCustomMetricStats, BinnedCustomMetrics},
    CustomMetricServerRecord, ServerRecords, StorageType, ToDriftRecords,
};
use std::sync::Arc;

pub struct CustomMetricDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for CustomMetricDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        CustomMetricDataFrame::new(storage_settings)
    }

    async fn get_dataframe(&self, records: ServerRecords) -> Result<DataFrame, DataFrameError> {
        let records = records.to_custom_metric_drift_records()?;
        let batch = self.build_batch(records)?;

        let ctx = self.object_store.get_session()?;

        let df = ctx.read_batches(vec![batch])?;

        Ok(df)
    }

    fn storage_root(&self) -> String {
        self.object_store.storage_settings.canonicalized_path()
    }

    fn storage_type(&self) -> StorageType {
        self.object_store.storage_settings.storage_type.clone()
    }

    fn get_session_context(&self) -> Result<SessionContext, DataFrameError> {
        Ok(self.object_store.get_session()?)
    }

    fn get_binned_sql(
        &self,
        bin: &f64,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
        space: &str,
        name: &str,
        version: &str,
    ) -> String {
        get_binned_custom_metric_values_query(bin, start_time, end_time, space, name, version)
    }

    fn table_name(&self) -> String {
        BinnedTableName::CustomMetric.to_string()
    }
}

impl CustomMetricDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("space", DataType::Utf8, false),
            Field::new("name", DataType::Utf8, false),
            Field::new("version", DataType::Utf8, false),
            Field::new("metric", DataType::Utf8, false),
            Field::new("value", DataType::Float64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(CustomMetricDataFrame {
            schema,
            object_store,
        })
    }

    fn build_batch(
        &self,
        records: Vec<CustomMetricServerRecord>,
    ) -> Result<RecordBatch, DataFrameError> {
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        let space_array = StringArray::from_iter_values(records.iter().map(|r| r.space.as_str()));
        let name_array = StringArray::from_iter_values(records.iter().map(|r| r.name.as_str()));
        let version_array =
            StringArray::from_iter_values(records.iter().map(|r| r.version.as_str()));
        let metric_array = StringArray::from_iter_values(records.iter().map(|r| r.metric.as_str()));

        let value_array = Float64Array::from_iter_values(records.iter().map(|r| r.value));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(space_array),
                Arc::new(name_array),
                Arc::new(version_array),
                Arc::new(metric_array),
                Arc::new(value_array),
            ],
        )?;

        Ok(batch)
    }
}

fn extract_created_at(batch: &RecordBatch) -> Result<Vec<DateTime<Utc>>, DataFrameError> {
    let created_at_list = batch
        .column(1)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::DowncastError("ListArray"))?;

    let created_at_array = created_at_list.value(0);
    Ok(created_at_array
        .as_primitive::<arrow::datatypes::TimestampNanosecondType>()
        .iter()
        .filter_map(|ts| ts.map(|t| Utc.timestamp_nanos(t)))
        .collect())
}

fn extract_stats(batch: &RecordBatch) -> Result<BinnedCustomMetricStats, DataFrameError> {
    let stats_list = batch
        .column(2)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::DowncastError("ListArray"))?
        .value(0);

    let stats_structs = stats_list
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| DataFrameError::DowncastError("StructArray"))?;

    // extract avg, lower_bound, and upper_bound from the struct

    // Extract avg, lower_bound, and upper_bound from the struct
    let avg = stats_structs
        .column_by_name("avg")
        .ok_or_else(|| DataFrameError::MissingFieldError("avg"))?
        .as_primitive::<Float64Type>()
        .value(0);

    let lower_bound = stats_structs
        .column_by_name("lower_bound")
        .ok_or_else(|| DataFrameError::MissingFieldError("lower_bound"))?
        .as_primitive::<Float64Type>()
        .value(0);

    let upper_bound = stats_structs
        .column_by_name("upper_bound")
        .ok_or_else(|| DataFrameError::MissingFieldError("upper_bound"))?
        .as_primitive::<Float64Type>()
        .value(0);

    Ok(BinnedCustomMetricStats {
        avg,
        lower_bound,
        upper_bound,
    })
}

fn process_custom_record_batch(batch: &RecordBatch) -> Result<BinnedCustomMetric, DataFrameError> {
    let metric_array = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringViewArray>()
        .expect("Failed to downcast to StringViewArray");
    let metric_name = metric_array.value(0).to_string();
    let created_at_list = extract_created_at(batch)?;
    let stats = extract_stats(batch)?;

    Ok(BinnedCustomMetric {
        metric: metric_name,
        created_at: created_at_list,
        stats: vec![stats],
    })
}

/// Convert a DataFrame to SpcDriftFeatures
///
/// # Arguments
/// * `df` - The DataFrame to convert
///
/// # Returns
/// * `SpcDriftFeatures` - The converted SpcDriftFeatures
pub async fn dataframe_to_custom_drift_metrics(
    df: DataFrame,
) -> Result<BinnedCustomMetrics, DataFrameError> {
    let batches = df.collect().await?;

    let metrics: Vec<BinnedCustomMetric> = batches
        .iter()
        .map(process_custom_record_batch)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(BinnedCustomMetrics::from_vec(metrics))
}
