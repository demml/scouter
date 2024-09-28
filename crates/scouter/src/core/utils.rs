use crate::core::error::ScouterError;
use colored_json::{Color, ColorMode, ColoredFormatter, PrettyFormatter, Styler};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyList, PyLong, PyString};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::path::PathBuf;

pub enum FileName {
    SpcDrift,
    Profile,
}

impl FileName {
    pub fn to_str(&self) -> &'static str {
        match self {
            FileName::SpcDrift => "Spc_drift_map.json",
            FileName::Profile => "data_profile.json",
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertDispatchType {
    Email,
    Slack,
    Console,
    OpsGenie,
}

#[pymethods]
impl AlertDispatchType {
    #[getter]
    pub fn value(&self) -> String {
        match self {
            AlertDispatchType::Email => "Email".to_string(),
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}

impl Default for AlertDispatchType {
    fn default() -> AlertDispatchType {
        AlertDispatchType::Console
    }
}

pub struct ProfileFuncs {}

impl ProfileFuncs {
    pub fn __str__<T: Serialize>(object: T) -> String {
        match ColoredFormatter::with_styler(
            PrettyFormatter::default(),
            Styler {
                key: Color::Rgb(245, 77, 85).bold(),
                string_value: Color::Rgb(249, 179, 93).foreground(),
                float_value: Color::Rgb(249, 179, 93).foreground(),
                integer_value: Color::Rgb(249, 179, 93).foreground(),
                bool_value: Color::Rgb(249, 179, 93).foreground(),
                nil_value: Color::Rgb(249, 179, 93).foreground(),
                ..Default::default()
            },
        )
        .to_colored_json(&object, ColorMode::On)
        {
            Ok(json) => json,
            Err(e) => format!("Failed to serialize to json: {}", e),
        }
        // serialize the struct to a string
    }

    pub fn __json__<T: Serialize>(object: T) -> String {
        match serde_json::to_string_pretty(&object) {
            Ok(json) => json,
            Err(e) => format!("Failed to serialize to json: {}", e),
        }
    }

    pub fn save_to_json<T>(
        model: T,
        path: Option<PathBuf>,
        filename: &str,
    ) -> Result<(), ScouterError>
    where
        T: Serialize,
    {
        // serialize the struct to a string
        let json =
            serde_json::to_string_pretty(&model).map_err(|_| ScouterError::SerializeError)?;

        // check if path is provided
        let write_path = if path.is_some() {
            let mut new_path = path.ok_or(ScouterError::CreatePathError)?;

            // ensure .json extension
            new_path.set_extension("json");

            if !new_path.exists() {
                // ensure path exists, create if not
                let parent_path = new_path.parent().ok_or(ScouterError::GetParentPathError)?;

                std::fs::create_dir_all(parent_path)
                    .map_err(|_| ScouterError::CreateDirectoryError)?;
            }

            new_path
        } else {
            PathBuf::from(filename)
        };

        std::fs::write(write_path, json).map_err(|_| ScouterError::WriteError)?;

        Ok(())
    }
}

pub fn json_to_pyobject(py: Python, value: &Value, dict: &PyDict) -> PyResult<()> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let py_value = match v {
                    Value::Null => py.None(),
                    Value::Bool(b) => b.into_py(py),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            i.into_py(py)
                        } else if let Some(f) = n.as_f64() {
                            f.into_py(py)
                        } else {
                            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                                "Invalid number",
                            ));
                        }
                    }
                    Value::String(s) => s.into_py(py),
                    Value::Array(arr) => {
                        let py_list = PyList::empty_bound(py);
                        for item in arr {
                            let py_item = json_to_pyobject_value(py, item)?;
                            py_list.append(py_item)?;
                        }
                        py_list.into_py(py)
                    }
                    Value::Object(_) => {
                        let nested_dict = PyDict::new_bound(py);
                        json_to_pyobject(py, v, nested_dict.as_gil_ref())?;
                        nested_dict.into_py(py)
                    }
                };
                dict.set_item(k, py_value)?;
            }
        }
        _ => {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Root must be an object",
            ))
        }
    }
    Ok(())
}

pub fn json_to_pyobject_value(py: Python, value: &Value) -> PyResult<PyObject> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py(py),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py(py)
            } else {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "Invalid number",
                ));
            }
        }
        Value::String(s) => s.into_py(py),
        Value::Array(arr) => {
            let py_list = PyList::empty_bound(py);
            for item in arr {
                let py_item = json_to_pyobject_value(py, item)?;
                py_list.append(py_item)?;
            }
            py_list.into_py(py)
        }
        Value::Object(_) => {
            let nested_dict = PyDict::new_bound(py);
            json_to_pyobject(py, value, nested_dict.as_gil_ref())?;
            nested_dict.into_py(py)
        }
    })
}

pub fn pyobject_to_json(_py: Python, obj: &PyAny) -> PyResult<Value> {
    if obj.is_instance_of::<PyDict>() {
        let dict = obj.downcast::<PyDict>()?;
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str = key.extract::<String>()?;
            let json_value = pyobject_to_json(_py, value)?;
            map.insert(key_str, json_value);
        }
        Ok(Value::Object(map))
    } else if obj.is_instance_of::<PyList>() {
        let list = obj.downcast::<PyList>()?;
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(pyobject_to_json(_py, item)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyString>() {
        let s = obj.extract::<String>()?;
        Ok(Value::String(s))
    } else if obj.is_instance_of::<PyFloat>() {
        let f = obj.extract::<f64>()?;
        Ok(json!(f))
    } else if obj.is_instance_of::<PyBool>() {
        let b = obj.extract::<bool>()?;
        Ok(json!(b))
    } else if obj.is_instance_of::<PyLong>() {
        let i = obj.extract::<i64>()?;
        Ok(json!(i))
    } else if obj.is_none() {
        Ok(Value::Null)
    } else {
        Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
            "Unsupported type: {}",
            obj.get_type().name()?
        )))
    }
}
