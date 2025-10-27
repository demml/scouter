use crate::queue::types::EntityType;
use crate::{error::TypeError, is_pydantic_model};
use crate::{json_to_pyobject_value, pyobject_to_json, PyHelperFuncs, Status};
use chrono::{DateTime, Utc};
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::{create_uuid7, Prompt};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Matches,
    HasLength,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssertionValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    List(Vec<AssertionValue>),
    Null(),
}

impl AssertionValue {
    pub fn to_actual(self, comparison: &ComparisonOperator) -> AssertionValue {
        match comparison {
            ComparisonOperator::HasLength => match self {
                AssertionValue::List(arr) => AssertionValue::Integer(arr.len() as i64),
                AssertionValue::String(s) => AssertionValue::Integer(s.chars().count() as i64),
                _ => self,
            },
            _ => self,
        }
    }
}

/// Converts a PyAny value to an AssertionValue
pub fn assertion_value_from_py(value: &Bound<'_, PyAny>) -> Result<AssertionValue, TypeError> {
    if let Ok(s) = value.extract::<String>() {
        Ok(AssertionValue::String(s))
    } else if let Ok(i) = value.extract::<i64>() {
        Ok(AssertionValue::Integer(i))
    } else if let Ok(f) = value.extract::<f64>() {
        Ok(AssertionValue::Number(f))
    } else if let Ok(b) = value.extract::<bool>() {
        Ok(AssertionValue::Boolean(b))
    } else if let Ok(list) = value.extract::<Vec<Bound<'_, PyAny>>>() {
        let converted_list: Result<Vec<_>, _> = list
            .iter()
            .map(|item| assertion_value_from_py(item))
            .collect();
        Ok(AssertionValue::List(converted_list?))
    } else {
        Err(TypeError::InvalidAssertionValueType)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMEvalMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,
}

#[pymethods]
impl LLMEvalMetric {
    #[new]
    #[pyo3(signature = (name, prompt))]
    pub fn new(name: &str, prompt: Prompt) -> Result<Self, TypeError> {
        // assert that the prompt is a scoring prompt
        if prompt.response_type != ResponseType::Score {
            return Err(TypeError::InvalidResponseType);
        }
        Ok(Self {
            name: name.to_lowercase(),
            prompt,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMJudgeTask {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,

    #[pyo3(get)]
    pub expected_value: AssertionValue,

    #[pyo3(get)]
    pub comparison: ComparisonOperator,
}

#[pymethods]
impl LLMJudgeTask {
    #[new]
    #[pyo3(signature = (name, prompt, expected_value, comparison))]
    pub fn new(
        name: &str,
        prompt: Prompt,
        expected_value: &Bound<'_, PyAny>,
        comparison: ComparisonOperator,
    ) -> Result<Self, TypeError> {
        let expected_value = assertion_value_from_py(expected_value)?;

        // assert that the response json schema is not empty
        if prompt.response_json_schema.is_none() {
            return Err(TypeError::EmptyJsonResponseSchema);
        }
        Ok(Self {
            name: name.to_lowercase(),
            prompt,
            expected_value,
            comparison,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

/// Converts a PyAny object to a serde_json::Value
/// This will first check if the object is a pydantic model and call model_dump if so
/// This is primarily used as a helper in instances when like LLMRecords that accept either dicts or pydantic models
/// # Arguments
/// * `py`: The Python GIL token
/// * `obj`: The PyAny object to convert
/// # Returns
/// A serde_json::Value representation of the object
fn convert_to_json(py: Python<'_>, obj: &Bound<'_, PyAny>) -> Result<Value, TypeError> {
    let is_pydantic = is_pydantic_model(py, obj)?;

    let value = if is_pydantic {
        let model = obj.call_method0("model_dump")?;
        pyobject_to_json(&model)?
    } else {
        pyobject_to_json(obj)?
    };

    Ok(value)
}

fn extract_value_from_py(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> Result<Value, TypeError> {
    match obj {
        Some(o) => Ok(convert_to_json(py, o)?),
        None => Ok(Value::Null),
    }
}

fn pydict_to_btreemap(dict: &Bound<'_, PyDict>) -> Result<BTreeMap<String, String>, TypeError> {
    let mut map = BTreeMap::new();
    for (key, value) in dict.iter() {
        let key_str: String = key.extract()?;
        let value_str: String = value.extract()?;
        map.insert(key_str, value_str);
    }
    Ok(map)
}
#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LLMEventRecord {
    pub id: i64,

    pub uid: String,

    pub space: String,

    pub name: String,

    pub version: String,

    pub created_at: DateTime<Utc>,

    pub prompt: Value,

    pub inputs: Value,

    pub outputs: Value,

    pub ground_truth: Option<Value>,

    pub metadata: BTreeMap<String, String>,

    #[pyo3(get)]
    pub entity_type: EntityType,

    pub root_id: String,

    pub event_id: String,

    pub event_name: String,

    pub parent_event_name: Option<String>,

    pub updated_at: Option<DateTime<Utc>>,

    pub processing_started_at: Option<DateTime<Utc>>,

    pub processing_ended_at: Option<DateTime<Utc>>,

    pub processing_duration: Option<i32>,

    pub status: Status,

    pub duration_ms: Option<i32>,
}

#[pymethods]
impl LLMEventRecord {
    #[new]
    #[pyo3(signature = (
        event_name,
        inputs=None,
        outputs=None,
        prompt=None,
        metadata=None,
        root_id=None,
        parent_event=None,
    ))]

    /// Creates a new LLMRecord instance.
    /// # Arguments
    /// * `inputs`: The inputs to the LLM as a Python object (dict,
    /// pydantic model, etc.)
    /// * `outputs`: The outputs from the LLM as a Python object (dict,
    /// pydantic model, etc.)
    /// * `prompt`: The prompt sent to the LLM
    /// * `metadata`: Optional metadata as a Python dictionary
    /// * `root_id`: Optional root ID for tracing
    /// * `parent_event_name`: Optional parent event name for tracing
    /// * `event_id`: Optional event ID for tracing
    ///
    pub fn new(
        py: Python<'_>,
        event_name: String,
        inputs: Option<Bound<'_, PyAny>>,
        outputs: Option<Bound<'_, PyAny>>,
        prompt: Option<Bound<'_, PyAny>>,
        metadata: Option<Bound<'_, PyDict>>,
        root_id: Option<String>,
        parent_event: Option<String>,
    ) -> Result<Self, TypeError> {
        // if inputs is None, set Value::Null
        let inputs_val: Value = extract_value_from_py(py, inputs.as_ref())?;
        let outputs_val: Value = extract_value_from_py(py, outputs.as_ref())?;

        // if inputs and outputs are both Null, raise an error
        if inputs_val == Value::Null && outputs_val == Value::Null {
            return Err(TypeError::EmptyInputsAndOutputs);
        }
        // if prompt is None, set Value::Null
        let prompt: Value = match prompt {
            Some(p) => {
                if p.is_instance_of::<Prompt>() {
                    let prompt = p.extract::<Prompt>()?;
                    serde_json::to_value(prompt)?
                } else {
                    pyobject_to_json(&p)?
                }
            }
            None => Value::Null,
        };

        // if metadata is None, set to empty BTreeMap
        let metadata_val: BTreeMap<String, String> = match metadata {
            Some(m) => pydict_to_btreemap(&m)?,
            None => BTreeMap::new(),
        };

        // if root_id is None, set to new uuid7
        let root_id = match root_id {
            Some(rid) => rid,
            None => create_uuid7(),
        };

        Ok(LLMEventRecord {
            id: 0, // this is a placeholder, the DB will set this
            uid: create_uuid7(),
            created_at: Utc::now(),
            space: String::new(),
            name: String::new(),
            version: String::new(),
            inputs: inputs_val,
            outputs: outputs_val,
            ground_truth: None,
            metadata: metadata_val,
            prompt,
            entity_type: EntityType::LLM,
            root_id,
            event_id: create_uuid7(),
            parent_event_name: parent_event,
            event_name,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
            status: Status::Pending,
            duration_ms: None,
        })
    }

    #[getter]
    pub fn inputs<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        Ok(json_to_pyobject_value(py, &self.inputs)?.bind(py).clone())
    }
}

impl LLMEventRecord {
    pub fn new_rs(
        space: String,
        name: String,
        version: String,
        uid: String,
        event_name: String,
        inputs: Option<Value>,
        outputs: Option<Value>,
        metadata: Option<BTreeMap<String, String>>,
        prompt: Option<Value>,
        root_id: Option<String>,
        parent_event: Option<String>,
    ) -> Self {
        LLMEventRecord {
            id: 0,
            space,
            name,
            version,
            uid,
            inputs: inputs.unwrap_or(Value::Null),
            outputs: outputs.unwrap_or(Value::Null),
            entity_type: EntityType::LLM,
            created_at: Utc::now(),
            prompt: prompt.unwrap_or(Value::Null),
            ground_truth: None,
            metadata: metadata.unwrap_or(BTreeMap::new()),
            root_id: root_id.unwrap_or(create_uuid7()),
            event_id: create_uuid7(),
            event_name,
            parent_event_name: parent_event,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
            status: Status::Pending,
            duration_ms: None,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}
