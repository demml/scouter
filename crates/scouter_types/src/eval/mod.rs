use crate::error::TypeError;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Prompt;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

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
