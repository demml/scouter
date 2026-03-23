use std::sync::{Arc, Mutex};

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};
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
/// Bound to a specific table via `TableConfig`. Validates the schema fingerprint
/// on construction. Supports strict reads (Pydantic models) and high-performance
/// SQL queries returning Arrow IPC bytes.
#[pyclass]
pub struct DatasetClient {
    client: Arc<Mutex<DatasetGrpcClient>>,
    namespace: DatasetNamespace,
    model_class: Py<PyAny>,
}

#[pymethods]
impl DatasetClient {
    #[new]
    #[instrument(skip_all)]
    fn new(
        py: Python<'_>,
        transport: &Bound<'_, PyAny>,
        table_config: &TableConfig,
    ) -> Result<Self, DatasetClientError> {
        let grpc_config = transport.extract::<GrpcConfig>().map_err(|_| {
            DatasetClientError::PyError("transport must be a GrpcConfig instance".to_string())
        })?;

        let namespace = table_config.namespace.clone();
        let expected_fp = table_config.fingerprint.as_str().to_string();

        // Create gRPC client and validate fingerprint
        let mut grpc_client =
            py.detach(|| app_state().block_on(DatasetGrpcClient::new(grpc_config)))?;

        // Validate fingerprint against server
        let describe_resp = py.detach(|| {
            app_state().block_on(grpc_client.describe_dataset(
                &namespace.catalog,
                &namespace.schema_name,
                &namespace.table,
            ))
        })?;

        if let Some(info) = &describe_resp.info {
            if info.fingerprint != expected_fp {
                return Err(DatasetClientError::FingerprintMismatch {
                    expected: expected_fp,
                    actual: info.fingerprint.clone(),
                });
            }
        }

        debug!(
            "DatasetClient initialized for {} (fingerprint: {})",
            namespace.fqn(),
            expected_fp
        );

        Ok(Self {
            client: Arc::new(Mutex::new(grpc_client)),
            namespace,
            model_class: table_config.model_class.clone_ref(py),
        })
    }

    /// Read all rows from the bound table as Pydantic model instances.
    ///
    /// Constructs `SELECT * FROM "catalog"."schema"."table"` internally,
    /// deserializes via pyarrow, and validates each row with the model class.
    #[pyo3(signature = (limit=None))]
    fn read<'py>(&self, py: Python<'py>, limit: Option<usize>) -> PyResult<Bound<'py, PyList>> {
        let fqn = self.namespace.fqn();
        let mut sql = format!("SELECT * FROM {fqn}");
        if let Some(n) = limit {
            sql.push_str(&format!(" LIMIT {n}"));
        }

        // Execute query and get IPC bytes
        let ipc_data = self.query_ipc(py, &sql)?;

        // Deserialize via pyarrow -> to_pydict -> model_validate
        let pa = py.import("pyarrow")?;
        let bytes = PyBytes::new(py, &ipc_data);
        let reader = pa.getattr("ipc")?.call_method1("open_stream", (bytes,))?;
        let table = reader.call_method0("read_all")?;

        let col_dict = table.call_method0("to_pydict")?;
        let col_names: Vec<String> = table.getattr("column_names")?.extract()?;
        let num_rows: usize = table.getattr("num_rows")?.extract()?;

        let results = PyList::empty(py);
        let model = self.model_class.bind(py);

        for i in 0..num_rows {
            let row_dict = PyDict::new(py);
            for col in &col_names {
                let col_values = col_dict.get_item(col)?;
                let value = col_values.get_item(i)?;
                row_dict.set_item(col, value)?;
            }
            let instance = model.call_method1("model_validate", (row_dict,))?;
            results.append(instance)?;
        }

        Ok(results)
    }

    /// Execute a SQL SELECT query and return a `QueryResult`.
    ///
    /// The `QueryResult` wraps Arrow IPC bytes and provides zero-copy
    /// conversion to `pyarrow.Table`, `polars.DataFrame`, or `pandas.DataFrame`.
    fn sql(&self, py: Python<'_>, query: String) -> Result<QueryResult, DatasetClientError> {
        let ipc_data = self.query_ipc(py, &query)?;
        Ok(QueryResult::new(ipc_data))
    }

    /// List all registered datasets.
    fn list_datasets<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let client = self.client.clone();
        let resp = py
            .detach(|| {
                let mut c = client.lock().unwrap();
                app_state().block_on(c.list_datasets())
            })
            .map_err(|e| DatasetClientError::GrpcError(e.to_string()))?;

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

    /// Describe a specific dataset (metadata + schema).
    #[pyo3(signature = (catalog, schema_name, table))]
    fn describe_dataset<'py>(
        &self,
        py: Python<'py>,
        catalog: String,
        schema_name: String,
        table: String,
    ) -> PyResult<Bound<'py, PyDict>> {
        let client = self.client.clone();
        let resp = py
            .detach(move || {
                let mut c = client.lock().unwrap();
                app_state().block_on(c.describe_dataset(&catalog, &schema_name, &table))
            })
            .map_err(|e| DatasetClientError::GrpcError(e.to_string()))?;

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
    /// Execute a SQL query via gRPC and return raw IPC bytes.
    fn query_ipc(&self, py: Python<'_>, query: &str) -> Result<Vec<u8>, DatasetClientError> {
        let client = self.client.clone();
        let query = query.to_string();
        let response = py.detach(move || {
            let mut c = client.lock().unwrap();
            app_state().block_on(c.query_dataset(&query))
        })?;
        Ok(response.ipc_data)
    }
}
