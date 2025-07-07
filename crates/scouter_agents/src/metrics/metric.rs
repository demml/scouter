use crate::error::MetricError;
use potato_head::{Agent, Prompt, Score, Workflow};
use potato_head::{StructuredOutput, Task};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// LLMMetric represents a metric for evaluating language model performance.
/// It's meant to provide maximum flexibility meaning that the
/// user can define any prompt they want to calculate their metric so long
/// as it return a Score.
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMEvalTask {
    pub eval_name: String,
    pub prompt: Prompt,
}

#[pymethods]
impl LLMEvalTask {
    #[new]
    pub fn new(eval_name: String, prompt: Prompt) -> Result<Self, MetricError> {
        // Validate required fields
        if eval_name.trim().is_empty() {
            return Err(MetricError::MissingField("eval_name".to_string()));
        }

        Ok(Self { eval_name, prompt })
    }
}

impl LLMEvalTask {
    pub async fn execute(&self) -> Result<Score, MetricError> {
        // get agent from prompt model settings
        let agent = Agent::from_model_settings(&self.prompt.model_settings)?;
        let response = agent.execute_prompt(&self.prompt).await?;

        let content = response.content();

        // if Value is string call Score::model_validate_json_str, else call Score::model_validate_json_value
        match content {
            serde_json::Value::String(s) => {
                Score::model_validate_json_str(&s).map_err(MetricError::from)
            }
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                Score::model_validate_json_value(&content).map_err(MetricError::from)
            }
            _ => Err(MetricError::ValidationError(
                "Invalid content type for Score".to_string(),
            )),
        }
    }
}

pub struct LLMEvaluationWorkflow {
    pub workflow: Workflow,
}

impl LLMEvaluationWorkflow {
    pub fn new(name: String) -> Result<Self, MetricError> {
        // Validate required fields
        let workflow = Workflow::new(&name);

        Ok(Self { workflow })
    }
}
