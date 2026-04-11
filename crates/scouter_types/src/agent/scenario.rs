use crate::agent::utils::{extract_assertion_tasks_from_pylist, AssertionTasks};
use crate::agent::{AgentAssertionTask, AssertionTask, LLMJudgeTask, TraceAssertionTask};
use crate::error::TypeError;
use crate::util::{json_to_pyobject, pyobject_to_json};
use crate::PyHelperFuncs;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

fn default_id() -> String {
    create_uuid7()
}

fn default_max_turns() -> usize {
    10
}

fn default_tasks() -> AssertionTasks {
    AssertionTasks {
        assertion: vec![],
        judge: vec![],
        trace: vec![],
        agent: vec![],
    }
}

/// A single test case in an offline agent evaluation run.
///
/// Scenarios drive `EvalScenarios` (the orchestrator). At minimum, supply an
/// `initial_query`. Everything else is optional.
///
/// ## Task attachment
/// Use `tasks` to attach scenario-level evaluation tasks (any of the four task
/// types: `AssertionTask`, `LLMJudgeTask`, `AgentAssertionTask`,
/// `TraceAssertionTask`). These are evaluated against the agent's **final
/// response** for this specific scenario. Sub-agent evaluation happens
/// holistically across all scenarios via the profiles already registered on
/// `ScouterQueue`.
///
/// ## Multi-turn scenarios
/// Populate `predefined_turns` with follow-up queries (executed in order after
/// `initial_query`). Leave empty for single-turn evaluation.
///
/// ## ReAct / simulated-user scenarios
/// `simulated_user_persona` and `termination_signal` are placeholder fields
/// for future ReAct support. Setting them has no effect in the current
/// implementation.
#[pyclass(from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalScenario {
    #[pyo3(get, set)]
    #[serde(default = "default_id")]
    pub id: String,

    #[pyo3(get, set)]
    pub initial_query: String,

    #[pyo3(get, set)]
    #[serde(default)]
    pub predefined_turns: Vec<String>,

    #[pyo3(get, set)]
    pub simulated_user_persona: Option<String>,

    #[pyo3(get, set)]
    pub termination_signal: Option<String>,

    #[pyo3(get, set)]
    #[serde(default = "default_max_turns")]
    pub max_turns: usize,

    #[pyo3(get, set)]
    pub expected_outcome: Option<String>,

    // Stored as structured buckets — same internal representation as GenAIEvalProfile.
    // Not exposed directly as a Python getter; use the typed getters below.
    #[serde(default = "default_tasks")]
    pub tasks: AssertionTasks,

    pub metadata: Option<HashMap<String, Value>>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl EvalScenario {
    #[new]
    #[pyo3(signature = (
        initial_query,
        tasks = None,
        id = None,
        expected_outcome = None,
        predefined_turns = None,
        simulated_user_persona = None,
        termination_signal = None,
        max_turns = 10,
        metadata = None,
    ))]
    pub fn new(
        initial_query: String,
        tasks: Option<&Bound<'_, PyList>>,
        id: Option<String>,
        expected_outcome: Option<String>,
        predefined_turns: Option<Vec<String>>,
        simulated_user_persona: Option<String>,
        termination_signal: Option<String>,
        max_turns: usize,
        metadata: Option<&Bound<'_, PyDict>>,
    ) -> Result<Self, TypeError> {
        let tasks = match tasks {
            Some(list) => extract_assertion_tasks_from_pylist(list)?,
            None => AssertionTasks {
                assertion: vec![],
                judge: vec![],
                trace: vec![],
                agent: vec![],
            },
        };

        let metadata = match metadata {
            Some(dict) => {
                let mut map = HashMap::new();
                for (k, v) in dict.iter() {
                    let key: String = k.extract()?;
                    let value: Value = pyobject_to_json(&v)?;
                    map.insert(key, value);
                }
                Some(map)
            }
            None => None,
        };

        Ok(Self {
            id: id.unwrap_or_else(create_uuid7),
            initial_query,
            predefined_turns: predefined_turns.unwrap_or_default(),
            simulated_user_persona,
            termination_signal,
            max_turns,
            expected_outcome,
            tasks,
            metadata,
        })
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        PyHelperFuncs::__json__(self)
    }

    pub fn model_dump(&self, py: Python) -> Result<Py<PyDict>, TypeError> {
        let json_value = serde_json::to_value(self)?;
        let dict = PyDict::new(py);
        json_to_pyobject(py, &json_value, &dict)?;
        Ok(dict.into())
    }

    #[getter]
    pub fn assertion_tasks(&self) -> Vec<AssertionTask> {
        self.tasks.assertion.clone()
    }

    #[getter]
    pub fn llm_judge_tasks(&self) -> Vec<LLMJudgeTask> {
        self.tasks.judge.clone()
    }

    #[getter]
    pub fn trace_assertion_tasks(&self) -> Vec<TraceAssertionTask> {
        self.tasks.trace.clone()
    }

    #[getter]
    pub fn agent_assertion_tasks(&self) -> Vec<AgentAssertionTask> {
        self.tasks.agent.clone()
    }

    #[getter]
    pub fn has_tasks(&self) -> bool {
        !self.tasks.assertion.is_empty()
            || !self.tasks.judge.is_empty()
            || !self.tasks.trace.is_empty()
            || !self.tasks.agent.is_empty()
    }

    /// Returns `true` when `predefined_turns` is non-empty (scripted multi-turn scenario).
    pub fn is_multi_turn(&self) -> bool {
        !self.predefined_turns.is_empty()
    }

    /// Returns `true` when a `simulated_user_persona` is set (ReAct/reactive scenario).
    ///
    /// Note: ReAct execution is deferred — this field is a placeholder until
    /// potato_head supports simulated user LLMs.
    pub fn is_reactive(&self) -> bool {
        self.simulated_user_persona.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_tasks() -> AssertionTasks {
        AssertionTasks {
            assertion: vec![],
            judge: vec![],
            trace: vec![],
            agent: vec![],
        }
    }

    #[test]
    fn default_values() {
        let s = EvalScenario {
            id: "test-id".to_string(),
            initial_query: "What is 2+2?".to_string(),
            predefined_turns: vec![],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 10,
            expected_outcome: None,
            tasks: empty_tasks(),
            metadata: None,
        };

        assert_eq!(s.max_turns, 10);
        assert!(!s.is_multi_turn());
        assert!(!s.is_reactive());
        assert!(!s.has_tasks());
    }

    #[test]
    fn multi_turn_detection() {
        let s = EvalScenario {
            id: "t".to_string(),
            initial_query: "Hello".to_string(),
            predefined_turns: vec!["Follow-up".to_string()],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 10,
            expected_outcome: None,
            tasks: empty_tasks(),
            metadata: None,
        };

        assert!(s.is_multi_turn());
        assert!(!s.is_reactive());
    }

    #[test]
    fn reactive_detection() {
        let s = EvalScenario {
            id: "t".to_string(),
            initial_query: "Hello".to_string(),
            predefined_turns: vec![],
            simulated_user_persona: Some("Busy professional".to_string()),
            termination_signal: Some("That's great".to_string()),
            max_turns: 8,
            expected_outcome: None,
            tasks: empty_tasks(),
            metadata: None,
        };

        assert!(!s.is_multi_turn());
        assert!(s.is_reactive());
    }

    #[test]
    fn serialization_roundtrip() {
        let s = EvalScenario {
            id: "roundtrip-id".to_string(),
            initial_query: "Make pasta".to_string(),
            predefined_turns: vec!["Make it spicier".to_string()],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 5,
            expected_outcome: Some("A pasta recipe".to_string()),
            tasks: empty_tasks(),
            metadata: None,
        };

        let json = serde_json::to_string(&s).unwrap();
        let restored: EvalScenario = serde_json::from_str(&json).unwrap();

        assert_eq!(s.id, restored.id);
        assert_eq!(s.initial_query, restored.initial_query);
        assert_eq!(s.predefined_turns, restored.predefined_turns);
        assert_eq!(s.max_turns, restored.max_turns);
        assert_eq!(s.expected_outcome, restored.expected_outcome);
    }

    #[test]
    fn default_id_from_serde() {
        // Deserializing from JSON without an `id` field should generate one.
        let json = r#"{"initial_query": "Hello"}"#;
        let s: EvalScenario = serde_json::from_str(json).unwrap();
        assert!(!s.id.is_empty());
        assert_eq!(s.max_turns, 10);
        assert!(s.predefined_turns.is_empty());
    }
}
