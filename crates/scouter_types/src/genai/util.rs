use crate::pyobject_to_json;
use crate::{error::TypeError, is_pydantic_model};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::collections::BTreeMap;

/// Converts a PyAny object to a serde_json::Value
/// This will first check if the object is a pydantic model and call model_dump if so
/// This is primarily used as a helper in instances when like LLMRecords that accept either dicts or pydantic models
/// # Arguments
/// * `py`: The Python GIL token
/// * `obj`: The PyAny object to convert
/// # Returns
/// A serde_json::Value representation of the object
pub fn convert_to_json(py: Python<'_>, obj: &Bound<'_, PyAny>) -> Result<Value, TypeError> {
    let is_pydantic = is_pydantic_model(py, obj)?;

    let value = if is_pydantic {
        let model = obj.call_method0("model_dump")?;
        pyobject_to_json(&model)?
    } else {
        pyobject_to_json(obj)?
    };

    Ok(value)
}

pub fn extract_value_from_py(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> Result<Value, TypeError> {
    match obj {
        Some(o) => Ok(convert_to_json(py, o)?),
        None => Ok(Value::Null),
    }
}

pub fn pydict_to_btreemap(dict: &Bound<'_, PyDict>) -> Result<BTreeMap<String, String>, TypeError> {
    let mut map = BTreeMap::new();
    for (key, value) in dict.iter() {
        let key_str: String = key.extract()?;
        let value_str: String = value.extract()?;
        map.insert(key_str, value_str);
    }
    Ok(map)
}
