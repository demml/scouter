use crate::api::state::AppState;
use chrono::Utc;

const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024; // 64 MB
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
            .max_decoding_message_size(MAX_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_MESSAGE_SIZE)
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

        let scenarios_json = req.scenarios_json.clone();
        let serialized_scenarios: Vec<(String, String)> =
            tokio::task::spawn_blocking(move || -> Result<Vec<(String, String)>, Status> {
                let scenarios: EvalScenarios = serde_json::from_str(&scenarios_json).map_err(|e| {
                    error!(error = %e, "Failed to deserialize EvalScenarios");
                    Status::invalid_argument("Invalid request payload")
                })?;
                scenarios
                    .scenarios
                    .iter()
                    .map(|s| {
                        serde_json::to_string(s)
                            .map(|json| (s.id.clone(), json))
                            .map_err(|e| {
                                error!(error = %e, scenario_id = %s.id, "Failed to serialize EvalScenario");
                                Status::internal("Failed to serialize scenario")
                            })
                    })
                    .collect()
            })
            .await
            .map_err(|e| Status::internal(e.to_string()))??;

        let collection_id = req.collection_id.clone();
        let created_at = Utc::now();
        let scenario_count = serialized_scenarios.len() as u64;

        let mut records: Vec<EvalScenarioRecord> = Vec::with_capacity(serialized_scenarios.len());
        for (scenario_id, scenario_json) in serialized_scenarios {
            records.push(EvalScenarioRecord {
                collection_id: collection_id.clone(),
                scenario_id,
                scenario_json,
                created_at,
            });
        }

        self.state
            .eval_scenario_service
            .write_scenarios(records)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to write eval scenarios");
                Status::internal("Internal error")
            })?;

        Ok(Response::new(RegisterScenariosResponse {
            status: "created".to_string(),
            collection_id,
            scenario_count,
        }))
    }
}
