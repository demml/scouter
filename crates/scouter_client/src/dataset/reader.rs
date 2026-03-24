use std::io::Cursor;
use std::sync::{Arc, Mutex};

use arrow::ipc::reader::StreamReader;
use arrow_json::ArrayWriter;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use scouter_settings::grpc::GrpcConfig;
use scouter_state::app_state;
use scouter_tonic::DatasetGrpcClient;
use scouter_types::dataset::DatasetNamespace;
use tracing::{debug, instrument};

use super::config::TableConfig;
use super::error::DatasetClientError;
use super::query_result::QueryResult;

/// Dataset client for reading and querying datasets.
///
/// When `table_config` is provided, validates the schema fingerprint on construction
/// and enables `read()` for Pydantic model deserialization.
/// When omitted, works as a general-purpose query client: `sql()`, `list_datasets()`,
/// and `describe_dataset()` all work without a table binding.
#[pyclass]
pub struct DatasetClient {
    client: Arc<Mutex<DatasetGrpcClient>>,
    namespace: Option<DatasetNamespace>,
    model_class: Option<Py<PyAny>>,
}

#[pymethods]
impl DatasetClient {
    #[new]
    #[pyo3(signature = (transport, table_config=None))]
    #[instrument(skip_all)]
    fn new(
        py: Python<'_>,
        transport: &Bound<'_, PyAny>,
        table_config: Option<&TableConfig>,
    ) -> Result<Self, DatasetClientError> {
        let grpc_config = transport.extract::<GrpcConfig>().map_err(|_| {
            DatasetClientError::PyError("transport must be a GrpcConfig instance".to_string())
        })?;

        let mut grpc_client =
            py.detach(|| app_state().block_on(DatasetGrpcClient::new(grpc_config)))?;

        let (namespace, model_class) = match table_config {
            Some(tc) => {
                let ns = tc.namespace.clone();
                let expected_fp = tc.fingerprint.as_str().to_string();

                let describe_resp = py.detach(|| {
                    app_state()
                        .block_on(grpc_client.describe_dataset(
                            &ns.catalog,
                            &ns.schema_name,
                            &ns.table,
                        ))
                        .map_err(DatasetClientError::from)
                })?;

                let info = describe_resp.info.ok_or_else(|| {
                    DatasetClientError::GrpcError(format!(
                        "Server returned no dataset info for '{}' -- table may not be registered",
                        ns.fqn()
                    ))
                })?;

                if info.fingerprint != expected_fp {
                    return Err(DatasetClientError::FingerprintMismatch {
                        expected: expected_fp,
                        actual: info.fingerprint,
                    });
                }

                debug!(
                    "DatasetClient initialized for {} (fingerprint: {})",
                    ns.fqn(),
                    expected_fp
                );

                (Some(ns), Some(tc.model_class.clone_ref(py)))
            }
            None => {
                debug!("DatasetClient initialized in unbound mode");
                (None, None)
            }
        };

        Ok(Self {
            client: Arc::new(Mutex::new(grpc_client)),
            namespace,
            model_class,
        })
    }

    /// Read all rows from the bound table as Pydantic model instances.
    ///
    /// Requires `table_config` to have been provided at construction.
    /// Deserializes Arrow IPC bytes via `arrow-json` and calls `model_validate_json` per row.
    #[pyo3(signature = (limit=None))]
    #[instrument(skip_all)]
    fn read<'py>(
        &self,
        py: Python<'py>,
        limit: Option<usize>,
    ) -> Result<Bound<'py, PyList>, DatasetClientError> {
        let namespace = self.namespace.as_ref().ok_or_else(|| {
            DatasetClientError::PyError(
                "read() requires a table_config — create DatasetClient(transport, table_config=TableConfig(...))"
                    .to_string(),
            )
        })?;
        let model_class = self.model_class.as_ref().ok_or_else(|| {
            DatasetClientError::PyError("read() requires a table_config".to_string())
        })?;

        let fqn = namespace.quoted_fqn();
        let mut sql = format!("SELECT * FROM {fqn}");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        let ipc_data = self.query_ipc(py, &sql)?;

        if ipc_data.is_empty() {
            return Ok(PyList::empty(py));
        }

        let cursor = Cursor::new(&ipc_data);
        let reader = StreamReader::try_new(cursor, None)?;
        let batches: Vec<_> = reader.collect::<Result<_, _>>()?;

        let mut buf: Vec<u8> = Vec::new();
        let mut writer = ArrayWriter::new(&mut buf);
        let batch_refs: Vec<&_> = batches.iter().collect();
        writer.write_batches(&batch_refs)?;
        writer.finish()?;

        let json_rows: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_slice(&buf)?;

        let results = PyList::empty(py);
        let model = model_class.bind(py);
        for row in &json_rows {
            let json_str = serde_json::to_string(row)?;
            let py_str = PyString::new(py, &json_str);
            let instance = model.call_method1("model_validate_json", (py_str,))?;
            results.append(instance)?;
        }

        Ok(results)
    }

    #[instrument(skip_all)]
    fn sql(&self, py: Python<'_>, query: String) -> Result<QueryResult, DatasetClientError> {
        let ipc_data = self.query_ipc(py, &query)?;
        Ok(QueryResult::new(ipc_data))
    }

    #[instrument(skip_all)]
    fn list_datasets<'py>(
        &self,
        py: Python<'py>,
    ) -> Result<Bound<'py, PyList>, DatasetClientError> {
        let client = self.client.clone();
        let resp = py.detach(|| {
            let mut c = client
                .lock()
                .map_err(|_| DatasetClientError::GrpcError("gRPC client lock poisoned".into()))?;
            app_state()
                .block_on(c.list_datasets())
                .map_err(DatasetClientError::from)
        })?;

        let results = PyList::empty(py);
        for info in &resp.datasets {
            let d = PyDict::new(py);
            d.set_item("catalog", &info.catalog)?;
            d.set_item("schema_name", &info.schema_name)?;
            d.set_item("table", &info.table)?;
            d.set_item("fingerprint", &info.fingerprint)?;
            d.set_item("partition_columns", &info.partition_columns)?;
            d.set_item("status", &info.status)?;
            d.set_item("created_at", &info.created_at)?;
            d.set_item("updated_at", &info.updated_at)?;
            results.append(d)?;
        }
        Ok(results)
    }

    #[pyo3(signature = (catalog, schema_name, table))]
    #[instrument(skip_all)]
    fn describe_dataset<'py>(
        &self,
        py: Python<'py>,
        catalog: String,
        schema_name: String,
        table: String,
    ) -> Result<Bound<'py, PyDict>, DatasetClientError> {
        let client = self.client.clone();
        let resp = py.detach(move || {
            let mut c = client
                .lock()
                .map_err(|_| DatasetClientError::GrpcError("gRPC client lock poisoned".into()))?;
            app_state()
                .block_on(c.describe_dataset(&catalog, &schema_name, &table))
                .map_err(DatasetClientError::from)
        })?;

        let d = PyDict::new(py);
        if let Some(info) = &resp.info {
            d.set_item("catalog", &info.catalog)?;
            d.set_item("schema_name", &info.schema_name)?;
            d.set_item("table", &info.table)?;
            d.set_item("fingerprint", &info.fingerprint)?;
            d.set_item("partition_columns", &info.partition_columns)?;
            d.set_item("status", &info.status)?;
            d.set_item("created_at", &info.created_at)?;
            d.set_item("updated_at", &info.updated_at)?;
        }
        d.set_item("arrow_schema_json", &resp.arrow_schema_json)?;
        Ok(d)
    }
}

impl DatasetClient {
    fn query_ipc(&self, py: Python<'_>, query: &str) -> Result<Vec<u8>, DatasetClientError> {
        let client = self.client.clone();
        let query = query.to_string();
        let response = py.detach(move || {
            let mut c = client
                .lock()
                .map_err(|_| DatasetClientError::GrpcError("gRPC client lock poisoned".into()))?;
            app_state()
                .block_on(c.query_dataset(&query))
                .map_err(DatasetClientError::from)
        })?;
        Ok(response.ipc_data)
    }
}
