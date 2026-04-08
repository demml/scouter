use crate::api::state::AppState;
use chrono::Utc;
use scouter_dataframe::EvalScenarioRecord;
use scouter_evaluate::scenario::EvalScenarios;
use scouter_tonic::{
    EvalScenarioService, EvalScenarioServiceServer, RegisterScenariosRequest,
    RegisterScenariosResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, instrument};

#[derive(Clone)]
pub struct EvalScenarioGrpcService {
    state: Arc<AppState>,
}

impl EvalScenarioGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn into_server(self) -> EvalScenarioServiceServer<Self> {
        EvalScenarioServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl EvalScenarioService for EvalScenarioGrpcService {
    #[instrument(skip_all)]
    async fn register_scenarios(
        &self,
        request: Request<RegisterScenariosRequest>,
    ) -> Result<Response<RegisterScenariosResponse>, Status> {
        let req = request.into_inner();

        let scenarios: EvalScenarios =
            serde_json::from_str(&req.scenarios_json).map_err(|e| {
                error!(error = %e, "Failed to deserialize EvalScenarios");
                Status::invalid_argument(format!("Invalid scenarios JSON: {e}"))
            })?;

        let collection_id = req.collection_id.clone();
        let created_at = Utc::now();
        let scenario_count = scenarios.scenarios.len() as u64;

        let records: Vec<EvalScenarioRecord> = scenarios
            .scenarios
            .iter()
            .map(|s| {
                let scenario_json = serde_json::to_string(s).unwrap_or_default();
                EvalScenarioRecord {
                    collection_id: collection_id.clone(),
                    scenario_id: s.id.clone(),
                    scenario_json,
                    created_at,
                }
            })
            .collect();

        self.state
            .eval_scenario_service
            .write_scenarios(records)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to write eval scenarios");
                Status::internal(format!("Failed to write scenarios: {e}"))
            })?;

        Ok(Response::new(RegisterScenariosResponse {
            status: "created".to_string(),
            collection_id,
            scenario_count,
        }))
    }
}
