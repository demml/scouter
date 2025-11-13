use crate::error::{ProfileError, TypeError, UtilError};
use crate::FeatureMap;
use crate::{CommonCrons, DriftType};
use base64::prelude::*;
use chrono::{DateTime, Utc};
use colored_json::{Color, ColorMode, ColoredFormatter, PrettyFormatter, Styler};
use opentelemetry::Key;
use opentelemetry::KeyValue;
use opentelemetry::Value as OTelValue;
use opentelemetry_proto::tonic::common::v1::{any_value::Value as AnyValueVariant, AnyValue};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString};
use pyo3::IntoPyObjectExt;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;

pub const MISSING: &str = "__missing__";
pub const DEFAULT_VERSION: &str = "0.0.0";

pub fn scouter_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

pub enum FileName {
    SpcDriftMap,
    SpcDriftProfile,
    PsiDriftMap,
    PsiDriftProfile,
    CustomDriftProfile,
    DriftProfile,
    DataProfile,
    LLMDriftProfile,
}

impl FileName {
    pub fn to_str(&self) -> &'static str {
        match self {
            FileName::SpcDriftMap => "spc_drift_map.json",
            FileName::SpcDriftProfile => "spc_drift_profile.json",
            FileName::PsiDriftMap => "psi_drift_map.json",
            FileName::PsiDriftProfile => "psi_drift_profile.json",
            FileName::CustomDriftProfile => "custom_drift_profile.json",
            FileName::DataProfile => "data_profile.json",
            FileName::DriftProfile => "drift_profile.json",
            FileName::LLMDriftProfile => "llm_drift_profile.json",
        }
    }
}

pub struct PyHelperFuncs {}

impl PyHelperFuncs {
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
            Err(e) => format!("Failed to serialize to json: {e}"),
        }
        // serialize the struct to a string
    }

    pub fn __json__<T: Serialize>(object: T) -> String {
        match serde_json::to_string_pretty(&object) {
            Ok(json) => json,
            Err(e) => format!("Failed to serialize to json: {e}"),
        }
    }

    pub fn save_to_json<T>(
        model: T,
        path: Option<PathBuf>,
        filename: &str,
    ) -> Result<PathBuf, UtilError>
    where
        T: Serialize,
    {
        // serialize the struct to a string
        let json = serde_json::to_string_pretty(&model)?;

        // check if path is provided
        let write_path = if path.is_some() {
            let mut new_path = path.ok_or(UtilError::CreatePathError)?;

            // ensure .json extension
            new_path.set_extension("json");

            if !new_path.exists() {
                // ensure path exists, create if not
                let parent_path = new_path.parent().ok_or(UtilError::GetParentPathError)?;

                std::fs::create_dir_all(parent_path)
                    .map_err(|_| UtilError::CreateDirectoryError)?;
            }

            new_path
        } else {
            PathBuf::from(filename)
        };

        std::fs::write(&write_path, json)?;

        Ok(write_path)
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
                            return Err(PyRuntimeError::new_err(
                                "Invalid number type, expected i64 or f64",
                            ));
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
        _ => return Err(PyRuntimeError::new_err("Root must be object")),
    }
    Ok(())
}

pub fn json_to_pyobject_value(py: Python, value: &Value) -> PyResult<Py<PyAny>> {
    Ok(match value {
        Value::Null => py.None(),
        Value::Bool(b) => b.into_py_any(py).unwrap(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py_any(py).unwrap()
            } else if let Some(f) = n.as_f64() {
                f.into_py_any(py).unwrap()
            } else {
                return Err(PyRuntimeError::new_err(
                    "Invalid number type, expected i64 or f64",
                ));
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

pub fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> Result<Value, TypeError> {
    if obj.is_instance_of::<PyDict>() {
        let dict = obj.downcast::<PyDict>()?;
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str = pyobject_to_json(&key)?;
            let json_value = pyobject_to_json(&value)?;
            map.insert(key_str.to_string(), json_value);
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
        Err(TypeError::UnsupportedPyObjectType)
    }
}

/// Converts a Python object to a tracing-compatible JSON Map, handling Pydantic BaseModel objects.
/// This will also truncate long string values to the specified max_length.
pub fn pyobject_to_tracing_json(
    obj: &Bound<'_, PyAny>,
    max_length: &usize,
) -> Result<Value, TypeError> {
    // check if object is pydantic basemodel
    let py = obj.py();

    if is_pydantic_basemodel(py, obj)? {
        let dict = obj.call_method0("model_dump")?;
        return pyobject_to_tracing_json(&dict, max_length);
    }
    if obj.is_instance_of::<PyDict>() {
        let dict = obj.downcast::<PyDict>()?;
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key = pyobject_to_tracing_json(&key, max_length)?;
            // match key to string
            let key_str = match key {
                Value::String(s) => s,
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => return Err(TypeError::InvalidDictKeyType),
            };
            let json_value = pyobject_to_tracing_json(&value, max_length)?;
            map.insert(key_str.to_string(), json_value);
        }
        Ok(Value::Object(map))
    } else if obj.is_instance_of::<PyList>() {
        let list = obj.downcast::<PyList>()?;
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(pyobject_to_tracing_json(&item, max_length)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyString>() {
        let s = obj.extract::<String>()?;
        let truncated = if s.len() > *max_length {
            format!("{}...[truncated]", &s[..*max_length])
        } else {
            s
        };
        Ok(Value::String(truncated))
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
        // return type as value
        // tracing should not fail because of unsupported type
        let ty = match obj.get_type().name() {
            Ok(name) => name.to_string(),
            Err(_) => "unknown".to_string(),
        };

        Ok(Value::String(ty))
    }
}

pub fn pyobject_to_otel_value(obj: &Bound<'_, PyAny>) -> Result<OTelValue, TypeError> {
    if obj.is_instance_of::<PyBool>() {
        let b = obj.extract::<bool>()?;
        Ok(OTelValue::Bool(b))
    } else if obj.is_instance_of::<PyInt>() {
        let i = obj.extract::<i64>()?;
        Ok(OTelValue::I64(i))
    } else if obj.is_instance_of::<PyFloat>() {
        let f = obj.extract::<f64>()?;
        Ok(OTelValue::F64(f))
    } else if obj.is_instance_of::<PyString>() {
        let s = obj.extract::<String>()?;
        Ok(OTelValue::String(opentelemetry::StringValue::from(s)))
    } else if obj.is_instance_of::<PyList>() {
        let list = obj.downcast::<PyList>()?;
        pylist_to_otel_array(list)
    } else if obj.is_none() {
        // Convert None to string "null" since OTEL doesn't have null
        Ok(OTelValue::String(opentelemetry::StringValue::from("null")))
    } else {
        let json_value = pyobject_to_json(obj)?;
        Ok(OTelValue::String(opentelemetry::StringValue::from(
            json_value.to_string(),
        )))
    }
}

fn flatten_nested_dict(
    obj: &Bound<'_, PyDict>,
    prefix: Option<String>,
) -> Result<Vec<KeyValue>, TypeError> {
    let mut result = Vec::new();

    for (key, value) in obj.iter() {
        let key_str = key.extract::<String>()?;
        let full_key = if let Some(ref p) = prefix {
            format!("{}.{}", p, key_str)
        } else {
            key_str
        };

        if value.is_instance_of::<PyDict>() {
            let nested_dict = value.downcast::<PyDict>()?;
            result.extend(flatten_nested_dict(nested_dict, Some(full_key))?);
        } else {
            let otel_value = pyobject_to_otel_value(&value)?;
            result.push(KeyValue::new(Key::new(full_key), otel_value));
        }
    }

    Ok(result)
}

pub fn pydict_to_otel_keyvalue(obj: &Bound<'_, PyDict>) -> Result<Vec<KeyValue>, TypeError> {
    flatten_nested_dict(obj, None)
}

/// Converts a Python list to an OpenTelemetry Array, ensuring homogeneous types.
fn pylist_to_otel_array(list: &Bound<'_, PyList>) -> Result<OTelValue, TypeError> {
    if list.is_empty() {
        // Return empty string array for empty lists
        return Ok(OTelValue::Array(opentelemetry::Array::String(Vec::new())));
    }

    // Check first element to determine array type
    let first_item = list.get_item(0)?;

    if first_item.is_instance_of::<PyBool>() {
        let mut bools = Vec::new();
        for item in list.iter() {
            if item.is_instance_of::<PyBool>() {
                bools.push(item.extract::<bool>()?);
            } else {
                // Mixed types - fall back to string array
                return pylist_to_string_array(list);
            }
        }
        Ok(OTelValue::Array(opentelemetry::Array::Bool(bools)))
    } else if first_item.is_instance_of::<PyInt>() {
        let mut ints = Vec::new();
        for item in list.iter() {
            if item.is_instance_of::<PyInt>() {
                ints.push(item.extract::<i64>()?);
            } else {
                // Mixed types - fall back to string array
                return pylist_to_string_array(list);
            }
        }
        Ok(OTelValue::Array(opentelemetry::Array::I64(ints)))
    } else if first_item.is_instance_of::<PyFloat>() {
        let mut floats = Vec::new();
        for item in list.iter() {
            if item.is_instance_of::<PyFloat>() {
                floats.push(item.extract::<f64>()?);
            } else {
                // Mixed types - fall back to string array
                return pylist_to_string_array(list);
            }
        }
        Ok(OTelValue::Array(opentelemetry::Array::F64(floats)))
    } else {
        // Default to string array for strings and mixed types
        pylist_to_string_array(list)
    }
}

/// Converts a Python list to a string array (fallback for mixed types).
fn pylist_to_string_array(list: &Bound<'_, PyList>) -> Result<OTelValue, TypeError> {
    let mut strings = Vec::new();
    for item in list.iter() {
        let string_val = if item.is_instance_of::<PyString>() {
            item.extract::<String>()?
        } else {
            // Convert other types to JSON string representation
            pyobject_to_json(&item)?.to_string()
        };
        strings.push(opentelemetry::StringValue::from(string_val));
    }
    Ok(OTelValue::Array(opentelemetry::Array::String(strings)))
}

/// Converts OpenTelemetry AnyValue to serde_json::Value
///
/// Handles all OpenTelemetry value types including nested arrays and key-value lists.
/// Invalid floating point values (NaN, infinity) are converted to null for JSON compatibility.
pub fn otel_value_to_serde_value(otel_value: &AnyValue) -> Value {
    match &otel_value.value {
        Some(variant) => match variant {
            AnyValueVariant::BoolValue(b) => Value::Bool(*b),
            AnyValueVariant::IntValue(i) => Value::Number(serde_json::Number::from(*i)),
            AnyValueVariant::DoubleValue(d) => {
                // Handle NaN and infinity cases gracefully
                serde_json::Number::from_f64(*d)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            AnyValueVariant::StringValue(s) => Value::String(s.clone()),
            AnyValueVariant::ArrayValue(array) => {
                let values: Vec<Value> =
                    array.values.iter().map(otel_value_to_serde_value).collect();
                Value::Array(values)
            }
            AnyValueVariant::KvlistValue(kvlist) => {
                let mut map = serde_json::Map::new();
                for kv in &kvlist.values {
                    if let Some(value) = &kv.value {
                        map.insert(kv.key.clone(), otel_value_to_serde_value(value));
                    }
                }
                Value::Object(map)
            }
            AnyValueVariant::BytesValue(bytes) => {
                // Convert bytes to base64 string for JSON compatibility
                Value::String(BASE64_STANDARD.encode(bytes))
            }
        },
        None => Value::Null,
    }
}

pub fn create_feature_map(
    features: &[String],
    array: &[Vec<String>],
) -> Result<FeatureMap, ProfileError> {
    // check if features and array are the same length
    if features.len() != array.len() {
        return Err(ProfileError::FeatureArrayLengthError);
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

/// Checks if python object is an instance of a Pydantic BaseModel
/// # Arguments
/// * `py` - Python interpreter instance
/// * `obj` - Python object to check
/// # Returns
/// * `Ok(bool)` - `true` if the object is a Pydantic model
/// * `Err(TypeError)` - if there was an error importing Pydantic or checking
pub fn is_pydantic_basemodel(py: Python, obj: &Bound<'_, PyAny>) -> Result<bool, TypeError> {
    let pydantic = match py.import("pydantic") {
        Ok(module) => module,
        // return false if pydantic cannot be imported
        Err(_) => return Ok(false),
    };

    let basemodel = pydantic.getattr("BaseModel")?;

    // check if context is a pydantic model
    let is_basemodel = obj
        .is_instance(&basemodel)
        .map_err(|e| TypeError::FailedToCheckPydanticModel(e.to_string()))?;

    Ok(is_basemodel)
}

pub fn is_pydict(obj: &Bound<'_, PyAny>) -> bool {
    obj.is_instance_of::<PyDict>()
}

/// Helper for converting a pydantic model to a pyobject (pydict)
/// we are keeping the type as Bound<'py, PyAny> so that is is compatible with pyobject_to_json
pub fn pydantic_to_value<'py>(obj: &Bound<'py, PyAny>) -> Result<Value, TypeError> {
    let dict = obj.call_method0("model_dump")?;
    pyobject_to_json(&dict)
}

#[derive(PartialEq, Debug)]
pub struct ProfileArgs {
    pub name: String,
    pub space: String,
    pub version: Option<String>,
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
    LLM,
}

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Pandas => write!(f, "pandas"),
            DataType::Polars => write!(f, "polars"),
            DataType::Numpy => write!(f, "numpy"),
            DataType::Arrow => write!(f, "arrow"),
            DataType::Unknown => write!(f, "unknown"),
            DataType::LLM => write!(f, "llm"),
        }
    }
}

impl DataType {
    pub fn from_module_name(module_name: &str) -> Result<Self, TypeError> {
        match module_name {
            "pandas.core.frame.DataFrame" => Ok(DataType::Pandas),
            "polars.dataframe.frame.DataFrame" => Ok(DataType::Polars),
            "numpy.ndarray" => Ok(DataType::Numpy),
            "pyarrow.lib.Table" => Ok(DataType::Arrow),
            "scouter_drift.llm.LLMRecord" => Ok(DataType::LLM),
            _ => Err(TypeError::InvalidDataType),
        }
    }
}

pub fn get_utc_datetime() -> DateTime<Utc> {
    Utc::now()
}

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertThreshold {
    Below,
    Above,
    Outside,
}

impl Display for AlertThreshold {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[pymethods]
impl AlertThreshold {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "below" => Some(AlertThreshold::Below),
            "above" => Some(AlertThreshold::Above),
            "outside" => Some(AlertThreshold::Outside),
            _ => None,
        }
    }

    pub fn __str__(&self) -> String {
        match self {
            AlertThreshold::Below => "Below".to_string(),
            AlertThreshold::Above => "Above".to_string(),
            AlertThreshold::Outside => "Outside".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub enum Status {
    #[default]
    All,
    Pending,
    Processing,
    Processed,
    Failed,
}

impl Status {
    pub fn as_str(&self) -> Option<&'static str> {
        match self {
            Status::All => None,
            Status::Pending => Some("pending"),
            Status::Processing => Some("processing"),
            Status::Processed => Some("processed"),
            Status::Failed => Some("failed"),
        }
    }
}

impl FromStr for Status {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(Status::All),
            "pending" => Ok(Status::Pending),
            "processing" => Ok(Status::Processing),
            "processed" => Ok(Status::Processed),
            "failed" => Ok(Status::Failed),
            _ => Err(TypeError::InvalidStatusError(s.to_string())),
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::All => write!(f, "all"),
            Status::Pending => write!(f, "pending"),
            Status::Processing => write!(f, "processing"),
            Status::Processed => write!(f, "processed"),
            Status::Failed => write!(f, "failed"),
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
