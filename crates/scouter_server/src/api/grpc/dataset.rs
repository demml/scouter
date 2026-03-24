use crate::api::state::AppState;
use scouter_dataframe::error::DatasetEngineError;
use scouter_dataframe::parquet::bifrost::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
use scouter_dataframe::parquet::bifrost::registry::RegistrationResult;
use scouter_tonic::{
    CancelQueryRequest, CancelQueryResponse, CatalogInfo, ColumnInfo, DatasetInfo, DatasetService,
    DatasetServiceServer, DescribeDatasetRequest, DescribeDatasetResponse, ExecuteQueryRequest,
    ExecuteQueryResponse, ExplainQueryRequest, ExplainQueryResponse, GetTableDetailRequest,
    GetTableDetailResponse, InsertBatchRequest, InsertBatchResponse, ListCatalogsRequest,
    ListCatalogsResponse, ListDatasetsRequest, ListDatasetsResponse, ListSchemasRequest,
    ListSchemasResponse, ListTablesRequest, ListTablesResponse, PlanNode, PlanNodeField,
    PlanNodeMetrics, PreviewTableRequest, PreviewTableResponse, QueryDatasetRequest,
    QueryDatasetResponse, QueryExecutionMetadata, RegisterDatasetRequest, RegisterDatasetResponse,
    SchemaInfo, TableStats, TableSummary,
};
use scouter_types::dataset::schema::{fingerprint_from_json_schema, inject_system_columns};
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
        DatasetEngineError::QueryCancelled(_) => Status::cancelled(e.to_string()),
        DatasetEngineError::DuplicateQueryId(_) => Status::already_exists(e.to_string()),
        _ => {
            error!("Dataset engine error: {:?}", e);
            Status::internal("Internal server error".to_string())
        }
    }
}

fn to_dataset_info(r: DatasetRegistration) -> DatasetInfo {
    DatasetInfo {
        catalog: r.namespace.catalog,
        schema_name: r.namespace.schema_name,
        table: r.namespace.table,
        fingerprint: r.fingerprint.as_str().to_string(),
        partition_columns: r.partition_columns,
        status: r.status.to_string(),
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

fn plan_node_to_proto(
    node: &scouter_dataframe::parquet::bifrost::explain::PlanNode,
) -> PlanNode {
    PlanNode {
        node_type: node.node_type.clone(),
        description: node.description.clone(),
        fields: node
            .fields
            .iter()
            .map(|f| PlanNodeField {
                key: f.key.clone(),
                value: f.value.clone(),
            })
            .collect(),
        children: node.children.iter().map(plan_node_to_proto).collect(),
        metrics: node.metrics.as_ref().map(|m| PlanNodeMetrics {
            output_rows: m.output_rows,
            elapsed_ms: m.elapsed_ms,
            bytes_scanned: m.bytes_scanned,
            spill_bytes: m.spill_bytes,
        }),
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

        let arrow_schema =
            scouter_types::dataset::schema::json_schema_to_arrow(&req.json_schema)
                .map_err(|e| Status::invalid_argument(format!("Invalid JSON schema: {e}")))?;

        let arrow_schema_with_sys = inject_system_columns(arrow_schema).map_err(|e| {
            Status::invalid_argument(format!("Failed to inject system columns: {e}"))
        })?;

        let arrow_schema_json = serde_json::to_string(&arrow_schema_with_sys)
            .map_err(|e| {
                error!("Failed to serialize Arrow schema: {e}");
                Status::internal("Internal server error")
            })?;

        let fingerprint = fingerprint_from_json_schema(&req.json_schema)
            .map_err(|e| Status::invalid_argument(format!("Failed to compute fingerprint: {e}")))?;

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
            .map_err(|e| {
                error!("Failed to serialize query results: {e}");
                Status::internal("Internal server error")
            })?;

        Ok(Response::new(QueryDatasetResponse { ipc_data }))
    }

    #[instrument(skip_all)]
    async fn list_datasets(
        &self,
        _request: Request<ListDatasetsRequest>,
    ) -> Result<Response<ListDatasetsResponse>, Status> {
        let datasets = self
            .state
            .dataset_manager
            .list_datasets()
            .into_iter()
            .map(to_dataset_info)
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

        let arrow_schema_json = registration.arrow_schema_json.clone();

        Ok(Response::new(DescribeDatasetResponse {
            info: Some(to_dataset_info(registration)),
            arrow_schema_json,
        }))
    }

    // ── Catalog Browser ─────────────────────────────────────────────

    #[instrument(skip_all)]
    async fn list_catalogs(
        &self,
        _request: Request<ListCatalogsRequest>,
    ) -> Result<Response<ListCatalogsResponse>, Status> {
        let catalogs = self
            .state
            .dataset_manager
            .list_catalogs()
            .into_iter()
            .map(|c| CatalogInfo {
                catalog: c.catalog,
                schema_count: c.schema_count,
                table_count: c.table_count,
            })
            .collect();

        Ok(Response::new(ListCatalogsResponse { catalogs }))
    }

    #[instrument(skip_all)]
    async fn list_schemas(
        &self,
        request: Request<ListSchemasRequest>,
    ) -> Result<Response<ListSchemasResponse>, Status> {
        let req = request.into_inner();

        let schemas = self
            .state
            .dataset_manager
            .list_schemas(&req.catalog)
            .into_iter()
            .map(|s| SchemaInfo {
                catalog: s.catalog,
                schema_name: s.schema_name,
                table_count: s.table_count,
            })
            .collect();

        Ok(Response::new(ListSchemasResponse { schemas }))
    }

    #[instrument(skip_all)]
    async fn list_tables(
        &self,
        request: Request<ListTablesRequest>,
    ) -> Result<Response<ListTablesResponse>, Status> {
        let req = request.into_inner();

        let tables = self
            .state
            .dataset_manager
            .list_tables(&req.catalog, &req.schema_name)
            .into_iter()
            .map(|t| TableSummary {
                catalog: t.catalog,
                schema_name: t.schema_name,
                table: t.table,
                status: t.status,
                created_at: t.created_at,
                updated_at: t.updated_at,
            })
            .collect();

        Ok(Response::new(ListTablesResponse { tables }))
    }

    #[instrument(skip_all)]
    async fn get_table_detail(
        &self,
        request: Request<GetTableDetailRequest>,
    ) -> Result<Response<GetTableDetailResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let detail = self
            .state
            .dataset_manager
            .get_table_detail(&namespace)
            .await
            .map_err(map_dataset_error)?;

        let columns = detail
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                arrow_type: c.arrow_type.clone(),
                nullable: c.nullable,
                is_partition: c.is_partition,
                is_system: c.is_system,
            })
            .collect();

        Ok(Response::new(GetTableDetailResponse {
            info: Some(to_dataset_info(detail.registration)),
            columns,
            stats: Some(TableStats {
                row_count: detail.stats.row_count,
                file_count: detail.stats.file_count,
                size_bytes: detail.stats.size_bytes,
                delta_version: detail.stats.delta_version,
            }),
        }))
    }

    #[instrument(skip_all)]
    async fn preview_table(
        &self,
        request: Request<PreviewTableRequest>,
    ) -> Result<Response<PreviewTableResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let max_rows = if req.max_rows == 0 { 50 } else { req.max_rows as usize };

        // Get column info from registration
        let detail = self
            .state
            .dataset_manager
            .get_table_detail(&namespace)
            .await
            .map_err(map_dataset_error)?;

        let columns: Vec<ColumnInfo> = detail
            .columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                arrow_type: c.arrow_type.clone(),
                nullable: c.nullable,
                is_partition: c.is_partition,
                is_system: c.is_system,
            })
            .collect();

        let batches = self
            .state
            .dataset_manager
            .preview_table(&namespace, max_rows)
            .await
            .map_err(map_dataset_error)?;

        let row_count: u64 = batches.iter().map(|b| b.num_rows() as u64).sum();

        let ipc_data = batches_to_ipc_bytes(&batches)
            .map_err(|e| {
                error!("Failed to serialize preview data: {e}");
                Status::internal("Internal server error")
            })?;

        Ok(Response::new(PreviewTableResponse {
            ipc_data,
            columns,
            row_count,
        }))
    }

    // ── Enhanced Query Execution ────────────────────────────────────

    #[instrument(skip_all)]
    async fn execute_query(
        &self,
        request: Request<ExecuteQueryRequest>,
    ) -> Result<Response<ExecuteQueryResponse>, Status> {
        let req = request.into_inner();

        let max_rows = if req.max_rows == 0 {
            1000
        } else {
            req.max_rows as usize
        };

        let result = self
            .state
            .dataset_manager
            .execute_query(&req.sql, &req.query_id, max_rows)
            .await
            .map_err(map_dataset_error)?;

        let ipc_data = batches_to_ipc_bytes(&result.batches)
            .map_err(|e| {
                error!("Failed to serialize query results: {e}");
                Status::internal("Internal server error")
            })?;

        Ok(Response::new(ExecuteQueryResponse {
            ipc_data,
            metadata: Some(QueryExecutionMetadata {
                query_id: result.metadata.query_id,
                rows_returned: result.metadata.rows_returned,
                truncated: result.metadata.truncated,
                execution_time_ms: result.metadata.execution_time_ms,
                bytes_scanned: result.metadata.bytes_scanned,
            }),
        }))
    }

    #[instrument(skip_all)]
    async fn cancel_query(
        &self,
        request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        let req = request.into_inner();
        let cancelled = self.state.dataset_manager.cancel_query(&req.query_id).await;
        Ok(Response::new(CancelQueryResponse { cancelled }))
    }

    // ── Query Plan ──────────────────────────────────────────────────

    #[instrument(skip_all)]
    async fn explain_query(
        &self,
        request: Request<ExplainQueryRequest>,
    ) -> Result<Response<ExplainQueryResponse>, Status> {
        let req = request.into_inner();

        let max_rows = if req.max_rows == 0 {
            1000
        } else {
            req.max_rows as usize
        };

        let result = self
            .state
            .dataset_manager
            .explain_query(&req.sql, req.analyze, max_rows)
            .await
            .map_err(map_dataset_error)?;

        Ok(Response::new(ExplainQueryResponse {
            logical_plan: Some(plan_node_to_proto(&result.logical_plan)),
            physical_plan: Some(plan_node_to_proto(&result.physical_plan)),
            logical_plan_text: result.logical_plan_text,
            physical_plan_text: result.physical_plan_text,
            execution_metadata: result.execution_metadata.map(|m| QueryExecutionMetadata {
                query_id: m.query_id,
                rows_returned: m.rows_returned,
                truncated: m.truncated,
                execution_time_ms: m.execution_time_ms,
                bytes_scanned: m.bytes_scanned,
            }),
        }))
    }
}
