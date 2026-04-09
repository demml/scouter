use crate::error::EvalScenarioEngineError;
use crate::parquet::eval_scenarios::engine::{
    EvalScenarioRecord, COLLECTION_ID_COL, CREATED_AT_COL, EVAL_SCENARIO_TABLE_NAME,
    SCENARIO_ID_COL, SCENARIO_JSON_COL,
};
use arrow::array::{Array, StringArray, TimestampMicrosecondArray};
use arrow::compute::cast;
use arrow::datatypes::DataType;
use arrow_array::LargeStringArray;
use chrono::{TimeZone, Utc};
use datafusion::error::DataFusionError;
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::SessionContext;
use std::sync::Arc;
use tracing::instrument;

pub struct EvalScenarioQueries {
    ctx: Arc<SessionContext>,
}

impl EvalScenarioQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self { ctx }
    }

    #[instrument(skip(self))]
    pub async fn get_scenarios(
        &self,
        collection_id: &str,
    ) -> Result<Vec<EvalScenarioRecord>, EvalScenarioEngineError> {
        let df = match self.ctx.table(EVAL_SCENARIO_TABLE_NAME).await {
            Ok(df) => df,
            Err(datafusion::error::DataFusionError::Plan(ref msg))
                if msg.contains("not exist") || msg.contains("not found") =>
            {
                return Ok(vec![]);
            }
            Err(e) => return Err(EvalScenarioEngineError::DatafusionError(e)),
        }
        .filter(col(COLLECTION_ID_COL).eq(lit(collection_id)))?;

        let batches = df.collect().await?;
        let mut records = Vec::new();

        for batch in &batches {
            let get_str_col = |name: &'static str| -> Result<StringArray, EvalScenarioEngineError> {
                let col = batch.column_by_name(name).ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(format!(
                        "column '{name}' missing"
                    )))
                })?;
                let casted = cast(col.as_ref(), &DataType::Utf8).map_err(|e| {
                    EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(format!(
                        "cast '{name}': {e}"
                    )))
                })?;
                casted
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .cloned()
                    .ok_or_else(|| {
                        EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                            format!("column '{name}' wrong type after cast"),
                        ))
                    })
            };

            let get_large_str_col =
                |name: &'static str| -> Result<LargeStringArray, EvalScenarioEngineError> {
                    let col = batch.column_by_name(name).ok_or_else(|| {
                        EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                            format!("column '{name}' missing"),
                        ))
                    })?;
                    let casted = cast(col.as_ref(), &DataType::LargeUtf8).map_err(|e| {
                        EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                            format!("cast '{name}': {e}"),
                        ))
                    })?;
                    casted
                        .as_any()
                        .downcast_ref::<LargeStringArray>()
                        .cloned()
                        .ok_or_else(|| {
                            EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                                format!("column '{name}' wrong type after cast"),
                            ))
                        })
                };

            let collection_ids = get_str_col(COLLECTION_ID_COL)?;
            let scenario_ids = get_str_col(SCENARIO_ID_COL)?;
            let scenario_jsons = get_large_str_col(SCENARIO_JSON_COL)?;

            let ts_type =
                DataType::Timestamp(arrow::datatypes::TimeUnit::Microsecond, Some("UTC".into()));
            let created_ats = batch
                .column_by_name(CREATED_AT_COL)
                .ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(format!(
                        "column '{}' missing",
                        CREATED_AT_COL
                    )))
                })
                .and_then(|col| {
                    cast(col.as_ref(), &ts_type).map_err(|e| {
                        EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                            format!("cast '{}': {e}", CREATED_AT_COL),
                        ))
                    })
                })
                .and_then(|casted| {
                    casted
                        .as_any()
                        .downcast_ref::<TimestampMicrosecondArray>()
                        .cloned()
                        .ok_or_else(|| {
                            EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                                format!("column '{}' wrong type after cast", CREATED_AT_COL),
                            ))
                        })
                })?;

            for i in 0..batch.num_rows() {
                let created_at = Utc
                    .timestamp_micros(created_ats.value(i))
                    .single()
                    .ok_or_else(|| {
                        EvalScenarioEngineError::DatafusionError(DataFusionError::Internal(
                            format!("invalid timestamp at row {i}"),
                        ))
                    })?;

                records.push(EvalScenarioRecord {
                    collection_id: collection_ids.value(i).to_string(),
                    scenario_id: scenario_ids.value(i).to_string(),
                    scenario_json: scenario_jsons.value(i).to_string(),
                    created_at,
                });
            }
        }

        Ok(records)
    }
}
