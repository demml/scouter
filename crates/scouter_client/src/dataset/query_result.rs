use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Wrapper around Arrow IPC stream bytes returned by a SQL query.
///
/// Provides zero-copy conversion to `pyarrow.Table`, `polars.DataFrame`,
/// and `pandas.DataFrame`. The IPC bytes are stored once; each conversion
/// reads from the same buffer.
#[pyclass(skip_from_py_object)]
pub struct QueryResult {
    ipc_data: Vec<u8>,
}

#[pymethods]
impl QueryResult {
    /// Convert to a `pyarrow.Table`. Requires `pyarrow`.
    fn to_arrow<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let pa = py.import("pyarrow")?;
        if self.ipc_data.is_empty() {
            // Zero-row result (empty table or LIMIT 0)
            // raises ArrowInvalid, so return an empty table instead.
            let empty_dict = pyo3::types::PyDict::new(py);
            return pa.call_method1("table", (empty_dict,));
        }
        let bytes = PyBytes::new(py, &self.ipc_data);
        let reader = pa.getattr("ipc")?.call_method1("open_stream", (bytes,))?;
        reader.call_method0("read_all")
    }

    /// Convert to a `polars.DataFrame` (zero-copy from Arrow).
    /// Requires `polars` and `pyarrow`.
    fn to_polars<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let arrow_table = self.to_arrow(py)?;
        let pl = py.import("polars")?;
        pl.call_method1("from_arrow", (arrow_table,))
    }

    /// Convert to a `pandas.DataFrame`. Requires `pyarrow`.
    fn to_pandas<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let arrow_table = self.to_arrow(py)?;
        arrow_table.call_method0("to_pandas")
    }

    /// Get the raw Arrow IPC stream bytes.
    fn to_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.ipc_data)
    }

    fn __len__(&self) -> usize {
        self.ipc_data.len()
    }

    fn __repr__(&self) -> String {
        format!("QueryResult({} bytes)", self.ipc_data.len())
    }
}

impl QueryResult {
    pub fn new(ipc_data: Vec<u8>) -> Self {
        Self { ipc_data }
    }
}
