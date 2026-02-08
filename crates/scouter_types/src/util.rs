use crate::error::{ProfileError, TypeError, UtilError};
use crate::traits::ConfigExt;
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
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::IntoPyObjectExt;
use pythonize::depythonize;
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
    GenAIEvalProfile,
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
            FileName::GenAIEvalProfile => "genai_drift_profile.json",
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
        let dict = obj.cast::<PyDict>()?;
        let mut map = serde_json::Map::new();
        for (key, value) in dict.iter() {
            let key_str = key.extract::<String>()?;
            let json_value = pyobject_to_json(&value)?;
            map.insert(key_str, json_value);
        }
        Ok(Value::Object(map))
    } else if obj.is_instance_of::<PyList>() {
        let list = obj.cast::<PyList>()?;
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(pyobject_to_json(&item)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyTuple>() {
        let tuple = obj.cast::<PyTuple>()?;
        let mut vec = Vec::new();
        for item in tuple.iter() {
            vec.push(pyobject_to_json(&item)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyBytes>() {
        let bytes = obj.cast::<PyBytes>()?;
        let b64_string = BASE64_STANDARD.encode(bytes.as_bytes());
        Ok(Value::String(b64_string))
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
        let dict = obj.cast::<PyDict>()?;
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
        let list = obj.cast::<PyList>()?;
        let mut vec = Vec::new();
        for item in list.iter() {
            vec.push(pyobject_to_tracing_json(&item, max_length)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyTuple>() {
        let tuple = obj.cast::<PyTuple>()?;
        let mut vec = Vec::new();
        for item in tuple.iter() {
            vec.push(pyobject_to_tracing_json(&item, max_length)?);
        }
        Ok(Value::Array(vec))
    } else if obj.is_instance_of::<PyBytes>() {
        let bytes = obj.cast::<PyBytes>()?;
        let b64_string = BASE64_STANDARD.encode(bytes.as_bytes());
        Ok(Value::String(b64_string))
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

/// Converts a Python object to an OpenTelemetry Value.
/// Complex types (dicts, nested structures) are serialized to JSON strings.
pub fn pyobject_to_otel_value(obj: &Bound<'_, PyAny>) -> Result<OTelValue, TypeError> {
    let value: Value = depythonize(obj)?;
    Ok(serde_value_to_otel_value(&value))
}

/// Converts serde_json::Value to OpenTelemetry Value.
/// Maps/objects are serialized to JSON strings since OTel doesn't support nested maps.
fn serde_value_to_otel_value(value: &Value) -> OTelValue {
    match value {
        Value::Bool(b) => OTelValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                OTelValue::I64(i)
            } else if let Some(f) = n.as_f64() {
                OTelValue::F64(f)
            } else {
                OTelValue::String(opentelemetry::StringValue::from(n.to_string()))
            }
        }
        Value::String(s) => OTelValue::String(opentelemetry::StringValue::from(s.clone())),
        Value::Array(arr) => {
            // Try to preserve homogeneous arrays, fallback to string array for mixed types
            if let Some(array) = try_homogeneous_array(arr) {
                OTelValue::Array(array)
            } else {
                // Mixed types - convert to string array
                let strings: Vec<opentelemetry::StringValue> = arr
                    .iter()
                    .map(|v| opentelemetry::StringValue::from(v.to_string()))
                    .collect();
                OTelValue::Array(opentelemetry::Array::String(strings))
            }
        }
        Value::Object(_) => {
            // OTel doesn't support nested maps - serialize to JSON string
            OTelValue::String(opentelemetry::StringValue::from(value.to_string()))
        }
        Value::Null => OTelValue::String(opentelemetry::StringValue::from("null")),
    }
}

/// Attempts to create a homogeneous OpenTelemetry array.
/// Returns None if the array contains mixed types.
fn try_homogeneous_array(arr: &[Value]) -> Option<opentelemetry::Array> {
    if arr.is_empty() {
        return Some(opentelemetry::Array::String(Vec::new()));
    }

    // Check if all elements are the same type
    match arr.first()? {
        Value::Bool(_) => {
            let bools: Option<Vec<bool>> = arr.iter().map(|v| v.as_bool()).collect();
            bools.map(opentelemetry::Array::Bool)
        }
        Value::Number(n) if n.is_i64() => {
            let ints: Option<Vec<i64>> = arr.iter().map(|v| v.as_i64()).collect();
            ints.map(opentelemetry::Array::I64)
        }
        Value::Number(_) => {
            let floats: Option<Vec<f64>> = arr.iter().map(|v| v.as_f64()).collect();
            floats.map(opentelemetry::Array::F64)
        }
        Value::String(_) => {
            let strings: Vec<opentelemetry::StringValue> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| opentelemetry::StringValue::from(s.to_string()))
                .collect();
            if strings.len() == arr.len() {
                Some(opentelemetry::Array::String(strings))
            } else {
                None
            }
        }
        _ => None, // Mixed types or nested structures
    }
}

/// Converts a Python dict to flattened OpenTelemetry KeyValue pairs.
/// Nested dicts are flattened using dot notation (e.g., "user.name", "user.email").
/// This is the standard approach for OpenTelemetry span events and attributes.
pub fn pydict_to_otel_keyvalue(obj: &Bound<'_, PyAny>) -> Result<Vec<KeyValue>, TypeError> {
    let value: Value = depythonize(obj)?;

    match value {
        Value::Object(map) => Ok(flatten_json_object(&map, None)),
        _ => Err(TypeError::ExpectedPyDict),
    }
}

/// Recursively flattens a JSON object into dot-notation key-value pairs
fn flatten_json_object(
    obj: &serde_json::Map<String, Value>,
    prefix: Option<&str>,
) -> Vec<KeyValue> {
    let mut result = Vec::new();

    for (key, value) in obj {
        let full_key = if let Some(p) = prefix {
            format!("{}.{}", p, key)
        } else {
            key.clone()
        };

        match value {
            Value::Object(nested) => {
                // Recursively flatten nested objects
                result.extend(flatten_json_object(nested, Some(&full_key)));
            }
            _ => {
                // Convert leaf values to OTel values
                let otel_value = serde_value_to_otel_value(value);
                result.push(KeyValue::new(Key::new(full_key), otel_value));
            }
        }
    }
    result
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
            AnyValueVariant::DoubleValue(d) => serde_json::Number::from_f64(*d)
                .map(Value::Number)
                .unwrap_or(Value::Null),
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
            AnyValueVariant::BytesValue(bytes) => Value::String(BASE64_STANDARD.encode(bytes)),
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
                map.insert(item.to_string(), j as i32);

                // check if j is last index
                if j == unique.len() - 1 {
                    // insert missing value
                    map.insert("missing".to_string(), j as i32 + 1);
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

fn process_dict_with_nested_models(
    py: Python<'_>,
    dict: &Bound<'_, PyAny>,
) -> Result<Value, TypeError> {
    let py_dict = dict.cast::<PyDict>()?;
    let mut result = serde_json::Map::new();

    for (key, value) in py_dict.iter() {
        let key_str: String = key.extract()?;
        let processed_value = depythonize_object_to_value(py, &value)?;
        result.insert(key_str, processed_value);
    }

    Ok(Value::Object(result))
}

pub fn depythonize_object_to_value<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> Result<Value, TypeError> {
    let py_value = if is_pydantic_basemodel(py, value)? {
        let model = value.call_method0("model_dump")?;
        depythonize(&model)?
    } else if value.is_instance_of::<PyDict>() {
        process_dict_with_nested_models(py, value)?
    } else {
        depythonize(value)?
    };
    Ok(py_value)
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
    type Config: ConfigExt;

    fn config(&self) -> &Self::Config;
    fn get_base_args(&self) -> ProfileArgs;
    fn to_value(&self) -> serde_json::Value;
    fn space(&self) -> &str {
        self.config().space()
    }
    fn name(&self) -> &str {
        self.config().name()
    }
    fn version(&self) -> &str {
        self.config().version()
    }
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

#[pyclass(eq, name = "ScouterDataType")]
#[derive(PartialEq, Debug)]
pub enum DataType {
    Pandas,
    Polars,
    Numpy,
    Arrow,
    Unknown,
    GenAI,
}

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::Pandas => write!(f, "pandas"),
            DataType::Polars => write!(f, "polars"),
            DataType::Numpy => write!(f, "numpy"),
            DataType::Arrow => write!(f, "arrow"),
            DataType::Unknown => write!(f, "unknown"),
            DataType::GenAI => write!(f, "genai"),
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
            "scouter_drift.genai.GenAIEvalRecord" => Ok(DataType::GenAI),
            _ => Err(TypeError::InvalidDataType),
        }
    }
}

pub fn get_utc_datetime() -> DateTime<Utc> {
    Utc::now()
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

impl TryFrom<String> for Status {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl std::str::FromStr for Status {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(Status::All),
            "pending" => Ok(Status::Pending),
            "processing" => Ok(Status::Processing),
            "processed" => Ok(Status::Processed),
            "failed" => Ok(Status::Failed),
            _ => Err(format!("Unknown status: {}", s)),
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
