use crate::FeatureMap;
use crate::{CommonCrons, DriftType};
use colored_json::{Color, ColorMode, ColoredFormatter, PrettyFormatter, Styler};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};
use pyo3::IntoPyObjectExt;
use rayon::prelude::*;
use scouter_error::ScouterError;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

pub const MISSING: &str = "__missing__";
pub const DEFAULT_VERSION: &str = "0.1.0";

pub enum FileName {
    SpcDrift,
    PsiDrift,
    Profile,
}

impl FileName {
    pub fn to_str(&self) -> &'static str {
        match self {
            FileName::SpcDrift => "spc_drift_map.json",
            FileName::PsiDrift => "psi_drift_map.json",
            FileName::Profile => "data_profile.json",
        }
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

pub fn json_to_pyobject(py: Python, value: &Value, dict: &Bound<'_, PyDict>) -> PyResult<()> {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let py_value = match v {
                    Value::Null => py.None(),
                    Value::Bool(b) => b.into_py_any(py).unwrap(),
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            i.into_py_any(py).unwrap()
                        } else if let Some(f) = n.as_f64() {
                            f.into_py_any(py).unwrap()
                        } else {
                            return Err(PyValueError::new_err("Invalid number"));
                        }
                    }
                    Value::String(s) => s.into_py_any(py).unwrap(),
                    Value::Array(arr) => {
                        let py_list = PyList::empty(py);
                        for item in arr {
                            let py_item = json_to_pyobject_value(py, item)?;
                            py_list.append(py_item)?;
                        }
                        py_list.into_py_any(py).unwrap()
                    }
                    Value::Object(_) => {
                        let nested_dict = PyDict::new(py);
                        json_to_pyobject(py, v, &nested_dict)?;
                        nested_dict.into_py_any(py).unwrap()
                    }
                };
                dict.set_item(k, py_value)?;
            }
        }
        _ => return Err(PyValueError::new_err("Root must be an object")),
    }
    Ok(())
}

pub fn json_to_pyobject_value(py: Python, value: &Value) -> PyResult<PyObject> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py_any(py).unwrap(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py_any(py).unwrap()
            } else if let Some(f) = n.as_f64() {
                f.into_py_any(py).unwrap()
            } else {
                return Err(PyValueError::new_err("Invalid number"));
            }
        }
        Value::String(s) => s.into_py_any(py).unwrap(),
        Value::Array(arr) => {
            let py_list = PyList::empty(py);
            for item in arr {
                let py_item = json_to_pyobject_value(py, item)?;
                py_list.append(py_item)?;
            }
            py_list.into_py_any(py).unwrap()
        }
        Value::Object(_) => {
            let nested_dict = PyDict::new(py);
            json_to_pyobject(py, value, &nested_dict)?;
            nested_dict.into_py_any(py).unwrap()
        }
    })
}

pub fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_instance_of::<PyDict>() {
        let dict = obj.downcast::<PyDict>()?;
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str = key.extract::<String>()?;
            let json_value = pyobject_to_json(&value)?;
            map.insert(key_str, json_value);
        }
        Ok(Value::Object(map))
    } else if obj.is_instance_of::<PyList>() {
        let list = obj.downcast::<PyList>()?;
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(pyobject_to_json(&item)?);
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
    } else if obj.is_instance_of::<PyInt>() {
        let i = obj.extract::<i64>()?;
        Ok(json!(i))
    } else if obj.is_none() {
        Ok(Value::Null)
    } else {
        Err(PyTypeError::new_err(format!(
            "Unsupported type: {}",
            obj.get_type().name()?
        )))
    }
}

pub fn create_feature_map(
    features: &[String],
    array: &[Vec<String>],
) -> Result<FeatureMap, ScouterError> {
    // check if features and array are the same length
    if features.len() != array.len() {
        return Err(ScouterError::ShapeMismatchError(
            "Features and array are not the same length".to_string(),
        ));
    };

    let feature_map = array
        .par_iter()
        .enumerate()
        .map(|(i, col)| {
            let unique = col
                .iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut map = HashMap::new();
            for (j, item) in unique.iter().enumerate() {
                map.insert(item.to_string(), j);

                // check if j is last index
                if j == unique.len() - 1 {
                    // insert missing value
                    map.insert("missing".to_string(), j + 1);
                }
            }

            (features[i].to_string(), map)
        })
        .collect::<HashMap<_, _>>();

    Ok(FeatureMap {
        features: feature_map,
    })
}

#[derive(PartialEq, Debug)]
pub struct ProfileArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub schedule: String,
    pub scouter_version: String,
    pub drift_type: DriftType,
}

// trait to implement on all profile types
pub trait ProfileBaseArgs {
    fn get_base_args(&self) -> ProfileArgs;
    fn to_value(&self) -> serde_json::Value;
}

pub trait ValidateAlertConfig {
    fn resolve_schedule(schedule: &str) -> String {
        let default_schedule = CommonCrons::EveryDay.cron();

        cron::Schedule::from_str(schedule) // Pass by reference here
            .map(|_| schedule) // If valid, return the schedule
            .unwrap_or_else(|_| {
                tracing::error!("Invalid cron schedule, using default schedule");
                &default_schedule
            })
            .to_string()
    }
}

#[pyclass(eq)]
#[derive(PartialEq, Debug)]
pub enum DataType {
    Pandas,
    Polars,
    Numpy,
    Arrow,
    Unknown,
}

impl DataType {
    pub fn from_module_name(module_name: &str) -> Result<Self, ScouterError> {
        match module_name {
            "pandas.core.frame.DataFrame" => Ok(DataType::Pandas),
            "polars.dataframe.frame.DataFrame" => Ok(DataType::Polars),
            "numpy.ndarray" => Ok(DataType::Numpy),
            "pyarrow.lib.Table" => Ok(DataType::Arrow),
            _ => Err(ScouterError::Error("Invalid data type".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct TestStruct;
    impl ValidateAlertConfig for TestStruct {}

    #[test]
    fn test_resolve_schedule_base() {
        let valid_schedule = "0 0 5 * * *"; // Every day at 5:00 AM

        let result = TestStruct::resolve_schedule(valid_schedule);

        assert_eq!(result, "0 0 5 * * *".to_string());

        let invalid_schedule = "invalid_cron";

        let default_schedule = CommonCrons::EveryDay.cron();

        let result = TestStruct::resolve_schedule(invalid_schedule);

        assert_eq!(result, default_schedule);
    }
}
