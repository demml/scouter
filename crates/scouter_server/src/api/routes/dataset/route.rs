use crate::api::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use scouter_dataframe::error::DatasetEngineError;
use scouter_dataframe::parquet::bifrost::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
use scouter_dataframe::parquet::bifrost::registry::RegistrationResult;
use scouter_types::contracts::ScouterServerError;
use scouter_types::dataset::schema::{fingerprint_from_json_schema, inject_system_columns};
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
use std::sync::Arc;
use tracing::{error, instrument};

// ── Request / Response types ────────────────────────────────────────

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct RegisterDatasetHttpRequest {
    catalog: String,
    schema_name: String,
    table: String,
    json_schema: String,
    partition_columns: Option<Vec<String>>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct RegisterDatasetHttpResponse {
    status: String,
    fingerprint: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct QueryDatasetHttpRequest {
    sql: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct InsertBatchHttpResponse {
    rows_accepted: u64,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct DatasetInfoResponse {
    catalog: String,
    schema_name: String,
    table: String,
    fingerprint: String,
    partition_columns: Vec<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ListDatasetsResponse {
    datasets: Vec<DatasetInfoResponse>,
}

// ── Catalog browser types ───────────────────────────────────────────

#[derive(serde::Serialize, utoipa::ToSchema)]
struct CatalogInfoResponse {
    catalog: String,
    schema_count: u32,
    table_count: u32,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ListCatalogsResponse {
    catalogs: Vec<CatalogInfoResponse>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct SchemaInfoResponse {
    catalog: String,
    schema_name: String,
    table_count: u32,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ListSchemasResponse {
    schemas: Vec<SchemaInfoResponse>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct TableSummaryResponse {
    catalog: String,
    schema_name: String,
    table: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ListTablesResponse {
    tables: Vec<TableSummaryResponse>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct ColumnInfoResponse {
    name: String,
    arrow_type: String,
    nullable: bool,
    is_partition: bool,
    is_system: bool,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct TableStatsResponse {
    row_count: Option<u64>,
    file_count: Option<u64>,
    size_bytes: Option<u64>,
    delta_version: Option<u64>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct TableDetailResponse {
    info: DatasetInfoResponse,
    columns: Vec<ColumnInfoResponse>,
    stats: TableStatsResponse,
}

#[derive(serde::Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct PreviewQueryParams {
    max_rows: Option<u32>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct PreviewResponse {
    columns: Vec<ColumnInfoResponse>,
    rows: Vec<Vec<serde_json::Value>>,
    row_count: u64,
}

// ── Enhanced query types ────────────────────────────────────────────

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct ExecuteQueryHttpRequest {
    sql: String,
    query_id: Option<String>,
    max_rows: Option<u32>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct QueryMetadataResponse {
    query_id: String,
    rows_returned: u64,
    truncated: bool,
    execution_time_ms: u64,
    bytes_scanned: Option<u64>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ExecuteQueryHttpResponse {
    columns: Vec<ColumnInfoResponse>,
    rows: Vec<Vec<serde_json::Value>>,
    metadata: QueryMetadataResponse,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct CancelQueryHttpRequest {
    query_id: String,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CancelQueryHttpResponse {
    cancelled: bool,
}

// ── Explain types ───────────────────────────────────────────────────

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct ExplainQueryHttpRequest {
    sql: String,
    analyze: Option<bool>,
    max_rows: Option<u32>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct PlanNodeResponse {
    node_type: String,
    description: String,
    #[schema(value_type = Vec<Object>)]
    children: Vec<PlanNodeResponse>,
    metrics: Option<PlanNodeMetricsResponse>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
struct PlanNodeMetricsResponse {
    output_rows: Option<u64>,
    elapsed_ms: Option<f64>,
    bytes_scanned: Option<u64>,
    spill_bytes: Option<u64>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ExplainQueryHttpResponse {
    logical_plan: PlanNodeResponse,
    physical_plan: PlanNodeResponse,
    logical_plan_text: String,
    physical_plan_text: String,
    execution_metadata: Option<QueryMetadataResponse>,
}

// ── Error mapping ───────────────────────────────────────────────────

fn map_dataset_error(e: DatasetEngineError) -> (StatusCode, Json<ScouterServerError>) {
    let (status, msg) = match &e {
        DatasetEngineError::TableNotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        DatasetEngineError::FingerprintMismatch { .. } => {
            (StatusCode::PRECONDITION_FAILED, e.to_string())
        }
        DatasetEngineError::ChannelClosed => (StatusCode::SERVICE_UNAVAILABLE, e.to_string()),
        DatasetEngineError::DatasetError(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        DatasetEngineError::SqlValidationError(_) => (StatusCode::BAD_REQUEST, e.to_string()),
        DatasetEngineError::QueryCancelled(_) => (
            StatusCode::from_u16(499).unwrap_or(StatusCode::BAD_REQUEST),
            e.to_string(),
        ),
        DatasetEngineError::DuplicateQueryId(_) => (StatusCode::CONFLICT, e.to_string()),
        _ => {
            error!("Dataset engine error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        }
    };
    (status, Json(ScouterServerError::new(msg)))
}

fn bad_request(msg: String) -> (StatusCode, Json<ScouterServerError>) {
    (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(msg)))
}

fn internal_error(
    msg: &str,
    detail: impl std::fmt::Display,
) -> (StatusCode, Json<ScouterServerError>) {
    error!("{}: {}", msg, detail);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ScouterServerError::new("Internal server error".to_string())),
    )
}

// ── Helpers ─────────────────────────────────────────────────────────

fn to_dataset_info_response(r: &DatasetRegistration) -> DatasetInfoResponse {
    DatasetInfoResponse {
        catalog: r.namespace.catalog.clone(),
        schema_name: r.namespace.schema_name.clone(),
        table: r.namespace.table.clone(),
        fingerprint: r.fingerprint.as_str().to_string(),
        partition_columns: r.partition_columns.clone(),
        status: r.status.to_string(),
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

/// Convert Arrow RecordBatches to JSON rows for HTTP responses.
fn batches_to_json_rows(
    batches: &[arrow_array::RecordBatch],
) -> Result<(Vec<ColumnInfoResponse>, Vec<Vec<serde_json::Value>>), String> {
    if batches.is_empty() {
        return Ok((vec![], vec![]));
    }

    let schema = batches[0].schema();
    let columns: Vec<ColumnInfoResponse> = schema
        .fields()
        .iter()
        .map(|f| ColumnInfoResponse {
            name: f.name().clone(),
            arrow_type: format!("{}", f.data_type()),
            nullable: f.is_nullable(),
            is_partition: false,
            is_system: false,
        })
        .collect();

    let mut writer = arrow_json::ArrayWriter::new(Vec::new());
    for batch in batches {
        writer.write(batch).map_err(|e| e.to_string())?;
    }
    writer.finish().map_err(|e| e.to_string())?;
    let json_bytes = writer.into_inner();

    let json_rows: Vec<serde_json::Value> =
        serde_json::from_slice(&json_bytes).map_err(|e| e.to_string())?;

    let rows: Vec<Vec<serde_json::Value>> = json_rows
        .into_iter()
        .map(|row| {
            if let serde_json::Value::Object(map) = row {
                schema
                    .fields()
                    .iter()
                    .map(|f| {
                        map.get(f.name())
                            .cloned()
                            .unwrap_or(serde_json::Value::Null)
                    })
                    .collect()
            } else {
                vec![]
            }
        })
        .collect();

    Ok((columns, rows))
}

fn plan_node_to_response(
    node: &scouter_dataframe::parquet::bifrost::explain::PlanNode,
) -> PlanNodeResponse {
    PlanNodeResponse {
        node_type: node.node_type.clone(),
        description: node.description.clone(),
        children: node.children.iter().map(plan_node_to_response).collect(),
        metrics: node.metrics.as_ref().map(|m| PlanNodeMetricsResponse {
            output_rows: m.output_rows,
            elapsed_ms: m.elapsed_ms,
            bytes_scanned: m.bytes_scanned,
            spill_bytes: m.spill_bytes,
        }),
    }
}

// ── Handlers ────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/scouter/datasets/register",
    request_body = RegisterDatasetHttpRequest,
    responses(
        (status = 200, description = "Dataset registered", body = RegisterDatasetHttpResponse),
        (status = 400, description = "Invalid schema or namespace", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn register_dataset(
    State(data): State<Arc<AppState>>,
    Json(body): Json<RegisterDatasetHttpRequest>,
) -> Result<Json<RegisterDatasetHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&body.catalog, &body.schema_name, &body.table)
        .map_err(|e| bad_request(e.to_string()))?;

    let json_schema = body.json_schema.clone();
    let (_, arrow_schema_json, fingerprint) = tokio::task::spawn_blocking(
        move || -> Result<_, (StatusCode, Json<ScouterServerError>)> {
            let arrow_schema = scouter_types::dataset::schema::json_schema_to_arrow(&json_schema)
                .map_err(|e| bad_request(format!("Invalid JSON schema: {e}")))?;
            let schema_with_sys = inject_system_columns(arrow_schema)
                .map_err(|e| bad_request(format!("Failed to inject system columns: {e}")))?;
            let schema_json = serde_json::to_string(&schema_with_sys)
                .map_err(|e| internal_error("Failed to serialize Arrow schema", e))?;
            let fp = fingerprint_from_json_schema(&json_schema)
                .map_err(|e| bad_request(format!("Failed to compute fingerprint: {e}")))?;
            Ok((schema_with_sys, schema_json, fp))
        },
    )
    .await
    .map_err(|e| internal_error("spawn error", e))??;

    let registration = DatasetRegistration::new(
        namespace,
        fingerprint.clone(),
        arrow_schema_json,
        body.json_schema,
        body.partition_columns.unwrap_or_default(),
    );

    let result = data
        .dataset_manager
        .register_dataset(&registration)
        .await
        .map_err(map_dataset_error)?;

    let status = match result {
        RegistrationResult::Created => "created",
        RegistrationResult::AlreadyExists => "already_exists",
    };

    Ok(Json(RegisterDatasetHttpResponse {
        status: status.to_string(),
        fingerprint: fingerprint.as_str().to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/scouter/datasets/{catalog}/{schema}/{table}/records",
    params(
        ("catalog" = String, Path, description = "Catalog name"),
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name"),
    ),
    request_body(content = inline(Vec<u8>), description = "Arrow IPC bytes", content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Batch inserted", body = InsertBatchHttpResponse),
        (status = 400, description = "Invalid data", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn insert_batch(
    State(data): State<Arc<AppState>>,
    Path((catalog, schema_name, table)): Path<(String, String, String)>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<InsertBatchHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&catalog, &schema_name, &table)
        .map_err(|e| bad_request(e.to_string()))?;

    let fingerprint_str = headers
        .get("x-dataset-fingerprint")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| bad_request("Missing x-dataset-fingerprint header".to_string()))?;

    let fingerprint = DatasetFingerprint(fingerprint_str.to_string());

    let batches = tokio::task::spawn_blocking(move || ipc_bytes_to_batches(&body))
        .await
        .map_err(|e| internal_error("spawn error", e))?
        .map_err(|e| bad_request(format!("Invalid IPC data: {e}")))?;

    // TODO(perf): activate engine once outside loop; currently insert_batch activates on each call
    let mut total_rows: u64 = 0;
    for batch in batches {
        total_rows += batch.num_rows() as u64;
        data.dataset_manager
            .insert_batch(&namespace, &fingerprint, batch)
            .await
            .map_err(map_dataset_error)?;
    }

    Ok(Json(InsertBatchHttpResponse {
        rows_accepted: total_rows,
    }))
}

#[utoipa::path(
    post,
    path = "/scouter/datasets/sql",
    request_body = QueryDatasetHttpRequest,
    responses(
        (status = 200, description = "Query results as Arrow IPC bytes"),
        (status = 400, description = "Invalid SQL", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn query_dataset(
    State(data): State<Arc<AppState>>,
    Json(body): Json<QueryDatasetHttpRequest>,
) -> Result<(StatusCode, axum::body::Bytes), (StatusCode, Json<ScouterServerError>)> {
    let batches = data
        .dataset_manager
        .query(&body.sql)
        .await
        .map_err(map_dataset_error)?;

    let ipc_data = tokio::task::spawn_blocking(move || batches_to_ipc_bytes(&batches))
        .await
        .map_err(|e| internal_error("spawn error", e))?
        .map_err(|e| internal_error("Failed to serialize query results", e))?;

    Ok((StatusCode::OK, axum::body::Bytes::from(ipc_data)))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets",
    responses(
        (status = 200, description = "List of registered datasets", body = ListDatasetsResponse),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn list_datasets_handler(
    State(data): State<Arc<AppState>>,
) -> Result<Json<ListDatasetsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let datasets = data.dataset_manager.list_datasets();

    let items: Vec<DatasetInfoResponse> = datasets.iter().map(to_dataset_info_response).collect();

    Ok(Json(ListDatasetsResponse { datasets: items }))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets/{catalog}/{schema}/{table}/info",
    params(
        ("catalog" = String, Path, description = "Catalog name"),
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name"),
    ),
    responses(
        (status = 200, description = "Dataset info", body = DatasetInfoResponse),
        (status = 404, description = "Dataset not found", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn get_dataset_info(
    State(data): State<Arc<AppState>>,
    Path((catalog, schema_name, table)): Path<(String, String, String)>,
) -> Result<Json<DatasetInfoResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&catalog, &schema_name, &table)
        .map_err(|e| bad_request(e.to_string()))?;

    let info = data
        .dataset_manager
        .get_dataset_info(&namespace)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ScouterServerError::new(format!(
                    "Dataset not found: {}",
                    namespace.fqn()
                ))),
            )
        })?;

    Ok(Json(to_dataset_info_response(&info)))
}

// ── Catalog browser handlers ────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/scouter/datasets/catalogs",
    responses(
        (status = 200, description = "List of catalogs", body = ListCatalogsResponse),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn list_catalogs(
    State(data): State<Arc<AppState>>,
) -> Result<Json<ListCatalogsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let catalogs = data
        .dataset_manager
        .list_catalogs()
        .into_iter()
        .map(|c| CatalogInfoResponse {
            catalog: c.catalog,
            schema_count: c.schema_count,
            table_count: c.table_count,
        })
        .collect();

    Ok(Json(ListCatalogsResponse { catalogs }))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets/catalogs/{catalog}/schemas",
    params(("catalog" = String, Path, description = "Catalog name")),
    responses(
        (status = 200, description = "List of schemas", body = ListSchemasResponse),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn list_schemas(
    State(data): State<Arc<AppState>>,
    Path(catalog): Path<String>,
) -> Result<Json<ListSchemasResponse>, (StatusCode, Json<ScouterServerError>)> {
    let schemas = data
        .dataset_manager
        .list_schemas(&catalog)
        .into_iter()
        .map(|s| SchemaInfoResponse {
            catalog: s.catalog,
            schema_name: s.schema_name,
            table_count: s.table_count,
        })
        .collect();

    Ok(Json(ListSchemasResponse { schemas }))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets/catalogs/{catalog}/schemas/{schema}/tables",
    params(
        ("catalog" = String, Path, description = "Catalog name"),
        ("schema" = String, Path, description = "Schema name"),
    ),
    responses(
        (status = 200, description = "List of tables", body = ListTablesResponse),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn list_tables(
    State(data): State<Arc<AppState>>,
    Path((catalog, schema_name)): Path<(String, String)>,
) -> Result<Json<ListTablesResponse>, (StatusCode, Json<ScouterServerError>)> {
    let tables = data
        .dataset_manager
        .list_tables(&catalog, &schema_name)
        .into_iter()
        .map(|t| TableSummaryResponse {
            catalog: t.catalog,
            schema_name: t.schema_name,
            table: t.table,
            status: t.status,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    Ok(Json(ListTablesResponse { tables }))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets/catalogs/{catalog}/schemas/{schema}/tables/{table}",
    params(
        ("catalog" = String, Path, description = "Catalog name"),
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name"),
    ),
    responses(
        (status = 200, description = "Table detail", body = TableDetailResponse),
        (status = 404, description = "Table not found", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn get_table_detail(
    State(data): State<Arc<AppState>>,
    Path((catalog, schema_name, table)): Path<(String, String, String)>,
) -> Result<Json<TableDetailResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&catalog, &schema_name, &table)
        .map_err(|e| bad_request(e.to_string()))?;

    let detail = data
        .dataset_manager
        .get_table_detail(&namespace)
        .await
        .map_err(map_dataset_error)?;

    let columns = detail
        .columns
        .iter()
        .map(|c| ColumnInfoResponse {
            name: c.name.clone(),
            arrow_type: c.arrow_type.clone(),
            nullable: c.nullable,
            is_partition: c.is_partition,
            is_system: c.is_system,
        })
        .collect();

    Ok(Json(TableDetailResponse {
        info: to_dataset_info_response(&detail.registration),
        columns,
        stats: TableStatsResponse {
            row_count: detail.stats.row_count,
            file_count: detail.stats.file_count,
            size_bytes: detail.stats.size_bytes,
            delta_version: detail.stats.delta_version,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/scouter/datasets/catalogs/{catalog}/schemas/{schema}/tables/{table}/preview",
    params(
        ("catalog" = String, Path, description = "Catalog name"),
        ("schema" = String, Path, description = "Schema name"),
        ("table" = String, Path, description = "Table name"),
        PreviewQueryParams,
    ),
    responses(
        (status = 200, description = "Table preview", body = PreviewResponse),
        (status = 404, description = "Table not found", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn preview_table(
    State(data): State<Arc<AppState>>,
    Path((catalog, schema_name, table)): Path<(String, String, String)>,
    Query(params): Query<PreviewQueryParams>,
) -> Result<Json<PreviewResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&catalog, &schema_name, &table)
        .map_err(|e| bad_request(e.to_string()))?;

    let max_rows = params.max_rows.unwrap_or(50).max(1) as usize;

    let batches = data
        .dataset_manager
        .preview_table(&namespace, max_rows)
        .await
        .map_err(map_dataset_error)?;

    let row_count: u64 = batches.iter().map(|b| b.num_rows() as u64).sum();
    let (columns, rows) = tokio::task::spawn_blocking(move || batches_to_json_rows(&batches))
        .await
        .map_err(|e| internal_error("spawn error", e))?
        .map_err(|e| internal_error("JSON serialization error", e))?;

    Ok(Json(PreviewResponse {
        columns,
        rows,
        row_count,
    }))
}

// ── Enhanced query handlers ─────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/scouter/datasets/query",
    request_body = ExecuteQueryHttpRequest,
    responses(
        (status = 200, description = "Query results", body = ExecuteQueryHttpResponse),
        (status = 400, description = "Invalid query", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn execute_query(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ExecuteQueryHttpRequest>,
) -> Result<Json<ExecuteQueryHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let max_rows = body.max_rows.unwrap_or(1000) as usize;
    let query_id = body
        .query_id
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    let result = data
        .dataset_manager
        .execute_query(&body.sql, &query_id, max_rows)
        .await
        .map_err(map_dataset_error)?;

    let batches = result.batches;
    let (columns, rows) = tokio::task::spawn_blocking(move || batches_to_json_rows(&batches))
        .await
        .map_err(|e| internal_error("spawn error", e))?
        .map_err(|e| internal_error("JSON serialization error", e))?;

    Ok(Json(ExecuteQueryHttpResponse {
        columns,
        rows,
        metadata: QueryMetadataResponse {
            query_id: result.metadata.query_id,
            rows_returned: result.metadata.rows_returned,
            truncated: result.metadata.truncated,
            execution_time_ms: result.metadata.execution_time_ms,
            bytes_scanned: result.metadata.bytes_scanned,
        },
    }))
}

#[utoipa::path(
    post,
    path = "/scouter/datasets/query/cancel",
    request_body = CancelQueryHttpRequest,
    responses(
        (status = 200, description = "Query cancellation result", body = CancelQueryHttpResponse),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn cancel_query(
    State(data): State<Arc<AppState>>,
    Json(body): Json<CancelQueryHttpRequest>,
) -> Result<Json<CancelQueryHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let cancelled = data.dataset_manager.cancel_query(&body.query_id).await;
    Ok(Json(CancelQueryHttpResponse { cancelled }))
}

// ── Explain handler ─────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/scouter/datasets/query/explain",
    request_body = ExplainQueryHttpRequest,
    responses(
        (status = 200, description = "Query execution plan", body = ExplainQueryHttpResponse),
        (status = 400, description = "Invalid query", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "datasets"
)]
#[instrument(skip_all)]
pub async fn explain_query(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ExplainQueryHttpRequest>,
) -> Result<Json<ExplainQueryHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let analyze = body.analyze.unwrap_or(false);
    let max_rows = body.max_rows.unwrap_or(1000) as usize;

    let result = data
        .dataset_manager
        .explain_query(&body.sql, analyze, max_rows)
        .await
        .map_err(map_dataset_error)?;

    Ok(Json(ExplainQueryHttpResponse {
        logical_plan: plan_node_to_response(&result.logical_plan),
        physical_plan: plan_node_to_response(&result.physical_plan),
        logical_plan_text: result.logical_plan_text,
        physical_plan_text: result.physical_plan_text,
        execution_metadata: result.execution_metadata.map(|m| QueryMetadataResponse {
            query_id: m.query_id,
            rows_returned: m.rows_returned,
            truncated: m.truncated,
            execution_time_ms: m.execution_time_ms,
            bytes_scanned: m.bytes_scanned,
        }),
    }))
}

// ── Router ──────────────────────────────────────────────────────────

pub fn get_dataset_router(prefix: &str) -> Router<Arc<AppState>> {
    Router::new()
        // Original dataset CRUD
        .route(
            &format!("{prefix}/datasets/register"),
            post(register_dataset),
        )
        .route(&format!("{prefix}/datasets/sql"), post(query_dataset))
        .route(&format!("{prefix}/datasets"), get(list_datasets_handler))
        .route(
            &format!("{prefix}/datasets/{{catalog}}/{{schema}}/{{table}}/records"),
            post(insert_batch),
        )
        .route(
            &format!("{prefix}/datasets/{{catalog}}/{{schema}}/{{table}}/info"),
            get(get_dataset_info),
        )
        // Catalog browser
        .route(
            &format!("{prefix}/datasets/catalogs"),
            get(list_catalogs),
        )
        .route(
            &format!("{prefix}/datasets/catalogs/{{catalog}}/schemas"),
            get(list_schemas),
        )
        .route(
            &format!("{prefix}/datasets/catalogs/{{catalog}}/schemas/{{schema}}/tables"),
            get(list_tables),
        )
        .route(
            &format!("{prefix}/datasets/catalogs/{{catalog}}/schemas/{{schema}}/tables/{{table}}"),
            get(get_table_detail),
        )
        .route(
            &format!("{prefix}/datasets/catalogs/{{catalog}}/schemas/{{schema}}/tables/{{table}}/preview"),
            get(preview_table),
        )
        // Enhanced query execution
        .route(&format!("{prefix}/datasets/query"), post(execute_query))
        .route(
            &format!("{prefix}/datasets/query/cancel"),
            post(cancel_query),
        )
        // Query plan
        .route(
            &format!("{prefix}/datasets/query/explain"),
            post(explain_query),
        )
}
