use crate::api::state::AppState;
use anyhow::Context;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use scouter_dataframe::error::DatasetEngineError;
use scouter_dataframe::parquet::dataset::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
use scouter_dataframe::parquet::dataset::registry::RegistrationResult;
use scouter_types::contracts::ScouterServerError;
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, instrument};

// ── Request / Response types ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RegisterDatasetHttpRequest {
    catalog: String,
    schema_name: String,
    table: String,
    json_schema: String,
    partition_columns: Option<Vec<String>>,
}

#[derive(serde::Serialize)]
struct RegisterDatasetHttpResponse {
    status: String,
    fingerprint: String,
}

#[derive(serde::Deserialize)]
struct QueryDatasetHttpRequest {
    sql: String,
}

#[derive(serde::Serialize)]
struct InsertBatchHttpResponse {
    rows_accepted: u64,
}

#[derive(serde::Serialize)]
struct DatasetInfoResponse {
    catalog: String,
    schema_name: String,
    table: String,
    fingerprint: String,
    partition_columns: Vec<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(serde::Serialize)]
struct ListDatasetsResponse {
    datasets: Vec<DatasetInfoResponse>,
}

// ── Error mapping ───────────────────────────────────────────────────

fn map_err(e: DatasetEngineError) -> (StatusCode, Json<ScouterServerError>) {
    let status = match &e {
        DatasetEngineError::TableNotFound(_) => StatusCode::NOT_FOUND,
        DatasetEngineError::FingerprintMismatch { .. } => StatusCode::PRECONDITION_FAILED,
        DatasetEngineError::ChannelClosed => StatusCode::SERVICE_UNAVAILABLE,
        DatasetEngineError::DatasetError(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error!("Dataset error: {:?}", e);
    (status, Json(ScouterServerError::new(e.to_string())))
}

fn bad_request(msg: String) -> (StatusCode, Json<ScouterServerError>) {
    (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(msg)))
}

fn internal_error(msg: String) -> (StatusCode, Json<ScouterServerError>) {
    error!("{}", msg);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ScouterServerError::new(msg)),
    )
}

// ── Handlers ────────────────────────────────────────────────────────

#[instrument(skip_all)]
async fn register_dataset(
    State(data): State<Arc<AppState>>,
    Json(body): Json<RegisterDatasetHttpRequest>,
) -> Result<Json<RegisterDatasetHttpResponse>, (StatusCode, Json<ScouterServerError>)> {
    let namespace = DatasetNamespace::new(&body.catalog, &body.schema_name, &body.table)
        .map_err(|e| bad_request(e.to_string()))?;

    let arrow_schema = scouter_types::dataset::schema::json_schema_to_arrow(&body.json_schema)
        .map_err(|e| bad_request(format!("Invalid JSON schema: {e}")))?;

    let arrow_schema_json = serde_json::to_string(&arrow_schema)
        .map_err(|e| internal_error(format!("Failed to serialize Arrow schema: {e}")))?;

    let fingerprint = DatasetFingerprint::from_schema_json(&arrow_schema_json);

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
        .map_err(map_err)?;

    let status = match result {
        RegistrationResult::Created => "created",
        RegistrationResult::AlreadyExists => "already_exists",
    };

    Ok(Json(RegisterDatasetHttpResponse {
        status: status.to_string(),
        fingerprint: fingerprint.as_str().to_string(),
    }))
}

#[instrument(skip_all)]
async fn insert_batch(
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

    let batches = ipc_bytes_to_batches(&body)
        .map_err(|e| bad_request(format!("Invalid IPC data: {e}")))?;

    let mut total_rows: u64 = 0;
    for batch in batches {
        total_rows += batch.num_rows() as u64;
        data.dataset_manager
            .insert_batch(&namespace, &fingerprint, batch)
            .await
            .map_err(map_err)?;
    }

    Ok(Json(InsertBatchHttpResponse {
        rows_accepted: total_rows,
    }))
}

#[instrument(skip_all)]
async fn query_dataset(
    State(data): State<Arc<AppState>>,
    Json(body): Json<QueryDatasetHttpRequest>,
) -> Result<(StatusCode, axum::body::Bytes), (StatusCode, Json<ScouterServerError>)> {
    let batches = data
        .dataset_manager
        .query(&body.sql)
        .await
        .map_err(map_err)?;

    let ipc_data = batches_to_ipc_bytes(&batches).map_err(|e| {
        internal_error(format!("Failed to serialize query results: {e}"))
    })?;

    Ok((StatusCode::OK, axum::body::Bytes::from(ipc_data)))
}

#[instrument(skip_all)]
async fn list_datasets(
    State(data): State<Arc<AppState>>,
) -> Result<Json<ListDatasetsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let datasets = data.dataset_manager.list_datasets();

    let items: Vec<DatasetInfoResponse> = datasets
        .into_iter()
        .map(|d| DatasetInfoResponse {
            catalog: d.namespace.catalog,
            schema_name: d.namespace.schema_name,
            table: d.namespace.table,
            fingerprint: d.fingerprint.as_str().to_string(),
            partition_columns: d.partition_columns,
            status: d.status.to_string(),
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListDatasetsResponse { datasets: items }))
}

#[instrument(skip_all)]
async fn get_dataset_info(
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

    Ok(Json(DatasetInfoResponse {
        catalog: info.namespace.catalog,
        schema_name: info.namespace.schema_name,
        table: info.namespace.table,
        fingerprint: info.fingerprint.as_str().to_string(),
        partition_columns: info.partition_columns,
        status: info.status.to_string(),
        created_at: info.created_at.to_rfc3339(),
        updated_at: info.updated_at.to_rfc3339(),
    }))
}

// ── Router ──────────────────────────────────────────────────────────

pub async fn get_dataset_router(
    prefix: &str,
) -> anyhow::Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(
                &format!("{prefix}/datasets/register"),
                post(register_dataset),
            )
            .route(&format!("{prefix}/datasets/sql"), post(query_dataset))
            .route(&format!("{prefix}/datasets"), get(list_datasets))
            .route(
                &format!("{prefix}/datasets/{{catalog}}/{{schema}}/{{table}}/records"),
                post(insert_batch),
            )
            .route(
                &format!("{prefix}/datasets/{{catalog}}/{{schema}}/{{table}}/info"),
                get(get_dataset_info),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => Err(anyhow::anyhow!("Failed to create dataset router"))
            .context("Panic occurred while creating the router"),
    }
}
