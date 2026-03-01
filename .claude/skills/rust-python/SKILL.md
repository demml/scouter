---
name: rust-python
description: Apply when working with Rust code, Python code, or PyO3 bindings in this codebase. Activates expert-level guidelines for writing interoperable Rust+Python code where Rust holds all logic and Python is a thin re-export layer.
---

You are an expert systems programmer specializing in high-performance Rust with Python interoperability via PyO3. Apply these guidelines to all code you write or review in this codebase.

## Core Philosophy

**All logic lives in Rust. Python is purely an interface.**

The Python layer does three things only:
1. Import compiled types/functions from the `_scouter` extension
2. Re-export them organized by domain
3. Initialize Rust-side logging on startup

This means Rust logic can be tested, benchmarked, and reused (e.g. by the server) without any Python dependency or runtime. Never put computation, validation, or business logic in Python.

---

## Architecture Layers

```
crates/scouter_*/          ← Pure Rust core logic (no pyo3 dependency)
crates/scouter_client/     ← Re-export hub + PyO3 wrapper types
py-scouter/src/            ← cdylib entry point; module registration
py-scouter/python/scouter/ ← Python re-export layer (zero logic)
stubs/*.pyi                ← Type hint stubs for IDE support
```

### Adding a new feature

1. Implement logic in the appropriate `crates/scouter_*/` crate with full Rust unit tests
2. Add `#[pyclass]` / `#[pymethods]` to types that need Python exposure
3. Re-export from `scouter_client/src/lib.rs`
4. Register the class in `py-scouter/src/<domain>.rs` via `m.add_class::<MyType>()?`
5. Add the import/re-export to the Python `__init__.py` for that domain
6. Update the relevant `.pyi` stub file; run `make build.stubs` to rebuild
7. Run `make setup.project` to rebuild the extension

---

## Rust Guidelines

### Standard Derives for Exposed Types

```rust
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MyProfile {
    #[pyo3(get)]           // read-only Python property
    pub name: String,
    #[pyo3(get, set)]      // read-write Python property
    pub version: String,
    pub internal_state: InternalType,  // not exposed — no pyo3 attribute
}
```

Always include:
- `Debug` — required for diagnostics
- `Clone` — enables pass-by-value across the boundary
- `Serialize, Deserialize` — enables `model_dump_json()` / `model_dump()`
- `PartialEq` — enables equality in tests and Python `==`

Use `#[pyclass(eq)]` when the type needs `==` support in Python.

### Constructors

Use `#[pyo3(signature = (...))]` to declare default/optional args. Validate and convert Python objects inside the constructor — the struct should always be internally consistent:

```rust
#[pymethods]
impl MyConfig {
    #[new]
    #[pyo3(signature = (space, name, version = "1.0.0", alert_config = None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        alert_config: Option<&Bound<'_, PyAny>>,
    ) -> Result<Self, TypeError> {
        let dispatch = match alert_config {
            None => AlertDispatchConfig::default(),
            Some(obj) => {
                if obj.is_instance_of::<SlackDispatchConfig>() {
                    AlertDispatchConfig::Slack(obj.extract::<SlackDispatchConfig>()?)
                } else {
                    AlertDispatchConfig::default()
                }
            }
        };
        Ok(Self { space: space.to_string(), name: name.to_string(), version: version.to_string(), dispatch })
    }
}
```

### Stateless Wrapper Pattern

High-level Python-facing classes (e.g. `Drifter`, `DataProfiler`) are stateless coordinators — they hold no data, only dispatch to internal Rust enums:

```rust
#[pyclass(name = "Drifter")]
#[derive(Debug, Default)]
pub struct PyDrifter {}

#[pymethods]
impl PyDrifter {
    #[new]
    pub fn new() -> Self { Self {} }

    pub fn create_drift_profile<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        // Extract config type, dispatch to internal Rust enum
        let drift_type = config
            .map(|c| c.getattr("drift_type")?.extract::<DriftType>())
            .transpose()?
            .unwrap_or_default();

        let result = internal_dispatch(py, data, drift_type)?;
        Ok(result.into_bound_py_any(py)?)
    }
}
```

### Returning Python Objects

Return `Bound<'py, PyAny>` when the concrete type is determined at runtime. Use `into_bound_py_any(py)?` to convert any `#[pyclass]` type back to Python:

```rust
match profile {
    DriftProfile::Spc(p)    => Ok(p.into_bound_py_any(py)?),
    DriftProfile::Psi(p)    => Ok(p.into_bound_py_any(py)?),
    DriftProfile::Custom(p) => Ok(p.into_bound_py_any(py)?),
}
```

### Python Object Inspection from Rust

When you need to read Python objects whose type is only known at runtime:

```rust
// Read attributes
let drift_type = obj.getattr("drift_type")?.extract::<DriftType>()?;

// Type-check then extract
if obj.is_instance_of::<SlackDispatchConfig>() {
    let cfg = obj.extract::<SlackDispatchConfig>()?;
}

// Introspect class name (for data type detection)
let module = data.getattr("__class__")?.getattr("__module__")?.str()?.to_string();
let name   = data.getattr("__class__")?.getattr("__name__")?.str()?.to_string();
```

### Heterogeneous Data Inputs (Pandas / NumPy / Polars / Arrow)

Use an enum-dispatched converter pattern to unify different Python data formats into a single Rust representation:

```rust
pub enum DataConverterEnum {
    Pandas(PandasDataConverter),
    Numpy(NumpyDataConverter),
    Polars(PolarsDataConverter),
    Arrow(ArrowDataConverter),
}

impl DataConverterEnum {
    pub fn convert<'py>(
        py: Python<'py>,
        data_type: &DataType,
        data: &Bound<'py, PyAny>,
    ) -> Result<ConvertedData<'py>, DataError> {
        match data_type {
            DataType::Pandas => PandasDataConverter::prepare_data(py, data),
            DataType::Numpy  => NumpyDataConverter::prepare_data(py, data),
            // ...
            _ => Err(DataError::UnsupportedDataTypeError(data_type.to_string())),
        }
    }
}
```

Infer the data type by inspecting `__module__` and `__class__.__name__`; then call Python methods (`.to_numpy()`, `.select_dtypes()`, etc.) from Rust to extract the data.

### Error Handling

Each domain has its own error enum. All errors implement `From<X> for PyErr` — use `PyRuntimeError` as the universal Python exception:

```rust
#[derive(Error, Debug)]
pub enum DriftError {
    #[error("{0}")]
    Error(String),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error(transparent)]
    TypeError(#[from] TypeError),
}

impl From<DriftError> for PyErr {
    fn from(err: DriftError) -> PyErr {
        PyRuntimeError::new_err(err.to_string())
    }
}

impl From<PyErr> for DriftError {
    fn from(err: PyErr) -> Self {
        DriftError::Error(err.to_string())
    }
}
```

### Pydantic-Compatible Serialization

All public types should support `model_dump_json()` and `model_dump()` for parity with Pydantic:

```rust
#[pymethods]
impl MyType {
    pub fn model_dump_json(&self) -> Result<String, TypeError> {
        serde_json::to_string(self).map_err(Into::into)
    }

    pub fn model_dump<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>, TypeError> {
        pythonize::to_pydict(py, self).map_err(Into::into)
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)  // serde_json pretty-print
    }
}
```

### Module Registration

Each domain registers its types in `py-scouter/src/<domain>.rs`:

```rust
pub fn add_drift_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(parent.py(), "drift")?;
    module.add_class::<SpcDriftConfig>()?;
    module.add_class::<SpcDriftProfile>()?;
    module.add_class::<PyDrifter>()?;
    // ...
    parent.add_submodule(&module)?;
    Ok(())
}
```

### Async

- Tokio multi-threaded runtime throughout
- Use `Arc<T>` + `RwLock<T>` for shared state
- When calling Python from a background thread, acquire the GIL explicitly: `Python::with_gil(|py| { ... })`

### Testing

Write Rust unit tests directly in `src/` — no Python required:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array;
    use rand::distributions::Uniform;
    use approx::assert_relative_eq;

    #[test]
    fn test_create_drift_profile() {
        let array = Array::random((1030, 3), Uniform::new(0., 10.).unwrap());
        let features = vec!["f1".to_string(), "f2".to_string(), "f3".to_string()];
        let config = SpcDriftConfig::new("space", "name", "1.0.0", None, None, None, None).unwrap();
        let monitor = SpcMonitor::new();
        let profile = monitor.create_2d_drift_profile(&features, &array.view(), &config).unwrap();
        assert_eq!(profile.features.len(), 3);
    }
}
```

- Use `approx::assert_relative_eq!` for float comparisons
- Use `ndarray::Array::random()` to generate test arrays
- SQL/integration tests use `--test-threads=1` for isolation
- `cargo clippy -- -D warnings` must pass cleanly

---

## Python Guidelines

### The Python Layer Is Re-exports Only

```python
# py-scouter/python/scouter/drift/__init__.py
from .._scouter import (
    Drifter,
    SpcDriftConfig,
    SpcDriftProfile,
    PsiDriftConfig,
    PsiDriftProfile,
)

__all__ = ["Drifter", "SpcDriftConfig", "SpcDriftProfile", "PsiDriftConfig", "PsiDriftProfile"]
```

No logic, no computation, no validation. If you find yourself writing a Python function that does work, move it to Rust.

### Type Hints

Maintain `.pyi` stub files in `py-scouter/stubs/`. After any public API change:
1. Update the relevant `.pyi` file
2. Run `make build.stubs` to assemble `_scouter.pyi`

Use the `header.pyi` type aliases and `BaseModel` protocol for consistency:

```python
SerializedType: TypeAlias = Union[str, int, float, dict, list]
Context: TypeAlias = Union[Dict[str, Any], "BaseModel"]
```

### Testing Python Bindings

Python tests verify that the binding works correctly, not that the algorithm is correct (that's Rust's job):

```python
import pytest
import numpy as np
from scouter import Drifter, SpcDriftConfig

def test_create_spc_profile():
    data = np.random.randn(1000, 3).astype(np.float64)
    config = SpcDriftConfig(space="test", name="model", version="1.0.0")
    profile = Drifter().create_drift_profile(data, config)
    assert len(profile.features) == 3

def test_model_dump_roundtrip():
    config = SpcDriftConfig(space="test", name="model", version="1.0.0")
    d = config.model_dump()
    assert d["name"] == "model"
```

### Tooling

- All commands use `uv run` inside `py-scouter/`
- Format: `isort` → `black` → `ruff`
- Lint: `ruff`, `pylint`, `mypy`
- After Rust changes: `make setup.project` (rebuilds the extension)

---

## Checklist for New Public APIs

- [ ] Logic implemented and unit-tested in the appropriate `crates/scouter_*/` crate
- [ ] Type has `#[pyclass]` + standard derives (`Debug, Clone, Serialize, Deserialize, PartialEq`)
- [ ] Constructor uses `#[pyo3(signature = (...))]` with sensible defaults
- [ ] `model_dump_json()`, `model_dump()`, `__str__()` implemented
- [ ] Error type has `impl From<MyError> for PyErr` and `impl From<PyErr> for MyError`
- [ ] Type re-exported from `scouter_client/src/lib.rs`
- [ ] Registered in `py-scouter/src/<domain>.rs` via `add_class`
- [ ] Python re-export added to the domain `__init__.py`
- [ ] `.pyi` stub updated; `make build.stubs` run
- [ ] Python-side test verifies the binding
- [ ] `make setup.project` run; `cargo clippy -- -D warnings` passes
