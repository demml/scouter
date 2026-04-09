use crate::error::EvalScenarioEngineError;
use crate::parquet::eval_scenarios::engine::{
    EvalScenarioDBEngine, EvalScenarioRecord, TableCommand,
};
use crate::parquet::eval_scenarios::queries::EvalScenarioQueries;
use scouter_settings::ObjectStorageSettings;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

pub struct EvalScenarioService {
    engine_tx: mpsc::Sender<TableCommand>,
    _engine_handle: tokio::task::JoinHandle<()>,
    pub query_service: EvalScenarioQueries,
}

impl EvalScenarioService {
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Self, EvalScenarioEngineError> {
        let engine = EvalScenarioDBEngine::new(storage_settings).await?;
        let ctx = engine.ctx();
        let (engine_tx, engine_handle) = engine.start_actor(24);

        info!("EvalScenarioService initialized");

        Ok(EvalScenarioService {
            engine_tx,
            _engine_handle: engine_handle,
            query_service: EvalScenarioQueries::new(ctx),
        })
    }

    pub async fn write_scenarios(
        &self,
        records: Vec<EvalScenarioRecord>,
    ) -> Result<(), EvalScenarioEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(TableCommand::Write {
                records,
                respond_to: tx,
            })
            .await
            .map_err(|_| EvalScenarioEngineError::ChannelClosed)?;

        rx.await
            .map_err(|_| EvalScenarioEngineError::ChannelClosed)?
    }

    pub async fn get_scenarios(
        &self,
        collection_id: &str,
    ) -> Result<Vec<EvalScenarioRecord>, EvalScenarioEngineError> {
        self.query_service.get_scenarios(collection_id).await
    }
}
