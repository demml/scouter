use crate::error::DataFrameError;
use crate::parquet::traits::ParquetFrame;
use crate::parquet::utils::ParquetHelper;
use crate::sql::helper::get_binned_psi_drift_records_query;
use crate::storage::ObjectStore;
use arrow::array::AsArray;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow_array::array::{
    ListArray, StringArray, StructArray, TimestampNanosecondArray, UInt64Array,
};
use arrow_array::types::{Float32Type, UInt64Type};
use arrow_array::RecordBatch;
use arrow_array::{Array, Int32Array};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use datafusion::dataframe::DataFrame;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::{
    psi::FeatureBinProportionResult, InternalServerRecords, PsiInternalRecord, StorageType,
    ToDriftRecords,
};
use std::collections::BTreeMap;
use std::sync::Arc;

use super::types::BinnedTableName;
pub struct PsiDataFrame {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
}

#[async_trait]
impl ParquetFrame for PsiDataFrame {
    fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        PsiDataFrame::new(storage_settings)
    }

    async fn get_dataframe(
        &self,
        records: InternalServerRecords,
    ) -> Result<DataFrame, DataFrameError> {
        let records = records.to_psi_drift_records()?;
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
        entity_id: &i32,
    ) -> String {
        get_binned_psi_drift_records_query(bin, start_time, end_time, entity_id)
    }

    fn table_name(&self) -> String {
        BinnedTableName::Psi.to_string()
    }
}

impl PsiDataFrame {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DataFrameError> {
        let schema = Arc::new(Schema::new(vec![
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new("entity_id", DataType::Int32, false),
            Field::new("feature", DataType::Utf8, false),
            Field::new("bin_id", DataType::UInt64, false),
            Field::new("bin_count", DataType::UInt64, false),
        ]));

        let object_store = ObjectStore::new(storage_settings)?;

        Ok(PsiDataFrame {
            schema,
            object_store,
        })
    }

    /// Create and arrow RecordBatch from the given records
    fn build_batch(&self, records: Vec<PsiInternalRecord>) -> Result<RecordBatch, DataFrameError> {
        let created_at_array = TimestampNanosecondArray::from_iter_values(
            records
                .iter()
                .map(|r| r.created_at.timestamp_nanos_opt().unwrap_or_default()),
        );

        let entity_id_array = Int32Array::from_iter_values(records.iter().map(|r| r.entity_id));
        let feature_array =
            StringArray::from_iter_values(records.iter().map(|r| r.feature.as_str()));

        let bin_id_array = UInt64Array::from_iter_values(records.iter().map(|r| r.bin_id as u64));
        let bin_count_array =
            UInt64Array::from_iter_values(records.iter().map(|r| r.bin_count as u64));

        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(created_at_array),
                Arc::new(entity_id_array),
                Arc::new(feature_array),
                Arc::new(bin_id_array),
                Arc::new(bin_count_array),
            ],
        )?;

        Ok(batch)
    }
}

/// Extraction logic to get bin proportions from a return record batch
fn get_bin_proportions_struct(batch: &RecordBatch) -> Result<&ListArray, DataFrameError> {
    batch
        .column(2)
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::GetColumnError("bin_proportions"))
}

/// Extraction logic to get bin ids and proportions from a return record batch
fn get_bin_fields(structs: &StructArray) -> Result<(&ListArray, &ListArray), DataFrameError> {
    let bin_ids = structs
        .column_by_name("bin_id")
        .ok_or_else(|| DataFrameError::MissingFieldError("bin_id"))?
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::GetColumnError("bin_id"))?;

    let proportions = structs
        .column_by_name("proportion")
        .ok_or_else(|| DataFrameError::MissingFieldError("proportion"))?
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::GetColumnError("proportion"))?;

    Ok((bin_ids, proportions))
}

/// Convert the bin id array to a Vec<usize>
fn get_bin_ids(array: &dyn Array) -> Result<Vec<usize>, DataFrameError> {
    Ok(array
        .as_primitive::<UInt64Type>()
        .iter()
        .filter_map(|id| id.map(|i| i as usize))
        .collect())
}

/// Convert the proportion array to a Vec<f64>
/// TODO: Should we store f64 or f32?
fn get_proportions(array: &dyn Array) -> Result<Vec<f64>, DataFrameError> {
    Ok(array
        .as_primitive::<Float32Type>()
        .iter()
        .filter_map(|p| p.map(|v| v as f64))
        .collect())
}

/// Create a BTreeMap from the bin ids and proportions
fn create_bin_map(
    bin_ids: &ListArray,
    proportions: &ListArray,
    index: usize,
) -> Result<BTreeMap<usize, f64>, DataFrameError> {
    let bin_ids = get_bin_ids(&bin_ids.value(index))?;
    let proportions = get_proportions(&proportions.value(index))?;

    Ok(bin_ids.into_iter().zip(proportions).collect())
}

/// Extract bin proportions from a return record batch
fn extract_bin_proportions(
    batch: &RecordBatch,
) -> Result<Vec<BTreeMap<usize, f64>>, DataFrameError> {
    let bin_structs = get_bin_proportions_struct(batch)?.value(0);
    let bin_structs = bin_structs
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| DataFrameError::DowncastError("Bin structs"))?;

    let (bin_ids_field, proportions_field) = get_bin_fields(bin_structs)?;

    let mut bin_proportions = Vec::with_capacity(bin_structs.len());
    for i in 0..bin_structs.len() {
        let bin_map = create_bin_map(bin_ids_field, proportions_field, i)?;
        bin_proportions.push(bin_map);
    }

    Ok(bin_proportions)
}

/// Extract overall proportions from a return record batch
fn get_overall_proportions_struct(batch: &RecordBatch) -> Result<&StructArray, DataFrameError> {
    let overall_proportions_struct = batch
        .column(3)
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| DataFrameError::DowncastError("overall proportion struct"))?;

    Ok(overall_proportions_struct)
}

fn get_overall_fields(
    overall_struct: &StructArray,
) -> Result<(&ListArray, &ListArray), DataFrameError> {
    let overall_bin_ids = overall_struct
        .column_by_name("bin_id")
        .ok_or_else(|| DataFrameError::MissingFieldError("bin_id"))?
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::DowncastError("bin_id"))?;

    let overall_proportions = overall_struct
        .column_by_name("proportion")
        .ok_or_else(|| DataFrameError::MissingFieldError("proportion"))?
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| DataFrameError::DowncastError("proporition"))?;

    Ok((overall_bin_ids, overall_proportions))
}

fn extract_overall_proportions(
    batch: &RecordBatch,
) -> Result<BTreeMap<usize, f64>, DataFrameError> {
    let overall_struct = get_overall_proportions_struct(batch)?;
    let (bin_ids, proportions) = get_overall_fields(overall_struct)?;

    let bin_ids = get_bin_ids(&bin_ids.value(0))?;
    let proportions = get_proportions(&proportions.value(0))?;

    Ok(bin_ids.into_iter().zip(proportions).collect())
}

/// Helper function to process a record batch to feature and SpcDriftFeature
///
/// # Arguments
/// * `batch` - The record batch to process
/// * `features` - The features to populate
///
/// # Returns
/// * `Result<(), DataFrameError>` - The result of the processing
fn process_psi_record_batch(
    batch: &RecordBatch,
) -> Result<FeatureBinProportionResult, DataFrameError> {
    Ok(FeatureBinProportionResult {
        feature: ParquetHelper::extract_feature_array(batch)?
            .value(0)
            .to_string(),
        created_at: ParquetHelper::extract_created_at(batch)?,
        bin_proportions: extract_bin_proportions(batch)?,
        overall_proportions: extract_overall_proportions(batch)?,
    })
}

/// Convert a DataFrame to SpcDriftFeatures
///
/// # Arguments
/// * `df` - The DataFrame to convert
///
/// # Returns
/// * `SpcDriftFeatures` - The converted SpcDriftFeatures
pub async fn dataframe_to_psi_drift_features(
    df: DataFrame,
) -> Result<Vec<FeatureBinProportionResult>, DataFrameError> {
    let batches = df.collect().await?;

    batches
        .into_iter()
        .map(|batch| process_psi_record_batch(&batch))
        .collect()
}
