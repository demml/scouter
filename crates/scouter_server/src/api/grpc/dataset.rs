use crate::api::state::AppState;
use scouter_dataframe::error::DatasetEngineError;
use scouter_dataframe::parquet::dataset::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
use scouter_dataframe::parquet::dataset::registry::RegistrationResult;
use scouter_tonic::{
    DatasetInfo, DatasetService, DatasetServiceServer, DescribeDatasetRequest,
    DescribeDatasetResponse, InsertBatchRequest, InsertBatchResponse, ListDatasetsRequest,
    ListDatasetsResponse, QueryDatasetRequest, QueryDatasetResponse, RegisterDatasetRequest,
    RegisterDatasetResponse,
};
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::{error, instrument};

const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024; // 64 MB

#[derive(Clone)]
pub struct DatasetGrpcService {
    state: Arc<AppState>,
}

impl DatasetGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn into_server(self) -> DatasetServiceServer<Self> {
        DatasetServiceServer::new(self)
            .max_decoding_message_size(MAX_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_MESSAGE_SIZE)
    }
}

fn map_dataset_error(e: DatasetEngineError) -> Status {
    match &e {
        DatasetEngineError::TableNotFound(_) => Status::not_found(e.to_string()),
        DatasetEngineError::FingerprintMismatch { .. } => {
            Status::failed_precondition(e.to_string())
        }
        DatasetEngineError::ChannelClosed => Status::unavailable(e.to_string()),
        DatasetEngineError::DatasetError(_) => Status::invalid_argument(e.to_string()),
        DatasetEngineError::SqlValidationError(_) => Status::invalid_argument(e.to_string()),
        _ => {
            error!("Dataset engine error: {:?}", e);
            Status::internal("Internal server error".to_string())
        }
    }
}

#[tonic::async_trait]
impl DatasetService for DatasetGrpcService {
    #[instrument(skip_all)]
    async fn register_dataset(
        &self,
        request: Request<RegisterDatasetRequest>,
    ) -> Result<Response<RegisterDatasetResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Parse Pydantic JSON schema -> Arrow schema on the server
        let arrow_schema =
            scouter_types::dataset::schema::json_schema_to_arrow(&req.json_schema)
                .map_err(|e| Status::invalid_argument(format!("Invalid JSON schema: {e}")))?;

        let arrow_schema_json = serde_json::to_string(&arrow_schema)
            .map_err(|e| Status::internal(format!("Failed to serialize Arrow schema: {e}")))?;

        let fingerprint = DatasetFingerprint::from_schema_json(&arrow_schema_json);

        let registration = DatasetRegistration::new(
            namespace,
            fingerprint.clone(),
            arrow_schema_json,
            req.json_schema,
            req.partition_columns,
        );

        let result = self
            .state
            .dataset_manager
            .register_dataset(&registration)
            .await
            .map_err(map_dataset_error)?;

        let status = match result {
            RegistrationResult::Created => "created",
            RegistrationResult::AlreadyExists => "already_exists",
        };

        Ok(Response::new(RegisterDatasetResponse {
            status: status.to_string(),
            fingerprint: fingerprint.as_str().to_string(),
        }))
    }

    #[instrument(skip_all)]
    async fn insert_batch(
        &self,
        request: Request<InsertBatchRequest>,
    ) -> Result<Response<InsertBatchResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let fingerprint = DatasetFingerprint(req.fingerprint);

        let batches = ipc_bytes_to_batches(&req.ipc_data)
            .map_err(|e| Status::invalid_argument(format!("Invalid IPC data: {e}")))?;

        let mut total_rows: u64 = 0;
        for batch in batches {
            total_rows += batch.num_rows() as u64;
            self.state
                .dataset_manager
                .insert_batch(&namespace, &fingerprint, batch)
                .await
                .map_err(map_dataset_error)?;
        }

        Ok(Response::new(InsertBatchResponse {
            rows_accepted: total_rows,
        }))
    }

    #[instrument(skip_all)]
    async fn query_dataset(
        &self,
        request: Request<QueryDatasetRequest>,
    ) -> Result<Response<QueryDatasetResponse>, Status> {
        let req = request.into_inner();

        let batches = self
            .state
            .dataset_manager
            .query(&req.sql)
            .await
            .map_err(map_dataset_error)?;

        let ipc_data = batches_to_ipc_bytes(&batches)
            .map_err(|e| Status::internal(format!("Failed to serialize query results: {e}")))?;

        Ok(Response::new(QueryDatasetResponse { ipc_data }))
    }

    #[instrument(skip_all)]
    async fn list_datasets(
        &self,
        _request: Request<ListDatasetsRequest>,
    ) -> Result<Response<ListDatasetsResponse>, Status> {
        let registrations = self.state.dataset_manager.list_datasets();

        let datasets = registrations
            .into_iter()
            .map(|r| DatasetInfo {
                catalog: r.namespace.catalog,
                schema_name: r.namespace.schema_name,
                table: r.namespace.table,
                fingerprint: r.fingerprint.as_str().to_string(),
                partition_columns: r.partition_columns,
                status: r.status.to_string(),
                created_at: r.created_at.to_rfc3339(),
                updated_at: r.updated_at.to_rfc3339(),
            })
            .collect();

        Ok(Response::new(ListDatasetsResponse { datasets }))
    }

    #[instrument(skip_all)]
    async fn describe_dataset(
        &self,
        request: Request<DescribeDatasetRequest>,
    ) -> Result<Response<DescribeDatasetResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let registration = self
            .state
            .dataset_manager
            .get_dataset_info(&namespace)
            .ok_or_else(|| Status::not_found(format!("Dataset not found: {}", namespace.fqn())))?;

        let info = DatasetInfo {
            catalog: registration.namespace.catalog,
            schema_name: registration.namespace.schema_name,
            table: registration.namespace.table,
            fingerprint: registration.fingerprint.as_str().to_string(),
            partition_columns: registration.partition_columns,
            status: registration.status.to_string(),
            created_at: registration.created_at.to_rfc3339(),
            updated_at: registration.updated_at.to_rfc3339(),
        };

        Ok(Response::new(DescribeDatasetResponse {
            info: Some(info),
            arrow_schema_json: registration.arrow_schema_json,
        }))
    }
}
