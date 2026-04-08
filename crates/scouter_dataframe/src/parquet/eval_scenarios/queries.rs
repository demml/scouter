use crate::error::EvalScenarioEngineError;
use crate::parquet::eval_scenarios::engine::{
    EvalScenarioRecord, COLLECTION_ID_COL, CREATED_AT_COL, EVAL_SCENARIO_TABLE_NAME,
    SCENARIO_ID_COL, SCENARIO_JSON_COL,
};
use arrow::array::{Array, StringArray, TimestampMicrosecondArray};
use arrow_array::LargeStringArray;
use chrono::{TimeZone, Utc};
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
        let df = self
            .ctx
            .table(EVAL_SCENARIO_TABLE_NAME)
            .await?
            .filter(col(COLLECTION_ID_COL).eq(lit(collection_id)))?;

        let batches = df.collect().await?;
        let mut records = Vec::new();

        for batch in &batches {
            let collection_ids = batch
                .column_by_name(COLLECTION_ID_COL)
                .and_then(|a| a.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(
                        datafusion::error::DataFusionError::Internal(format!(
                            "column '{}' missing or wrong type",
                            COLLECTION_ID_COL
                        )),
                    )
                })?;

            let scenario_ids = batch
                .column_by_name(SCENARIO_ID_COL)
                .and_then(|a| a.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(
                        datafusion::error::DataFusionError::Internal(format!(
                            "column '{}' missing or wrong type",
                            SCENARIO_ID_COL
                        )),
                    )
                })?;

            let scenario_jsons = batch
                .column_by_name(SCENARIO_JSON_COL)
                .and_then(|a| a.as_any().downcast_ref::<LargeStringArray>())
                .ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(
                        datafusion::error::DataFusionError::Internal(format!(
                            "column '{}' missing or wrong type",
                            SCENARIO_JSON_COL
                        )),
                    )
                })?;

            let created_ats = batch
                .column_by_name(CREATED_AT_COL)
                .and_then(|a| a.as_any().downcast_ref::<TimestampMicrosecondArray>())
                .ok_or_else(|| {
                    EvalScenarioEngineError::DatafusionError(
                        datafusion::error::DataFusionError::Internal(format!(
                            "column '{}' missing or wrong type",
                            CREATED_AT_COL
                        )),
                    )
                })?;

            for i in 0..batch.num_rows() {
                let created_at = Utc
                    .timestamp_micros(created_ats.value(i))
                    .single()
                    .unwrap_or_else(Utc::now);

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
