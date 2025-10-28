use crate::error::TypeError;
use crate::genai::util::{extract_value_from_py, pydict_to_btreemap};
use crate::queue::types::EntityType;
use crate::{json_to_pyobject_value, pyobject_to_json, PyHelperFuncs, Status};
use chrono::{DateTime, Utc};
use core::fmt::Debug;
use potato_head::{create_uuid7, Prompt};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationCursor {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationRequest {
    pub limit: i32,
    pub cursor: Option<PaginationCursor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationResponse<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<PaginationCursor>,
    pub has_more: bool,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct GenAIEventRecord {
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
impl GenAIEventRecord {
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

        Ok(GenAIEventRecord {
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
            entity_type: EntityType::GenAI,
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

impl GenAIEventRecord {
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
        GenAIEventRecord {
            id: 0,
            space,
            name,
            version,
            uid,
            inputs: inputs.unwrap_or(Value::Null),
            outputs: outputs.unwrap_or(Value::Null),
            entity_type: EntityType::GenAI,
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
