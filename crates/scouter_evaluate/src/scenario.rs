use crate::agent::EvalDataset;
use crate::error::EvaluationError;
use crate::evaluate::scenario_results::{EvalMetrics, ScenarioResult};
use crate::evaluate::types::EvalResults;
use potato_head::{create_uuid7, PyHelperFuncs};
use pyo3::prelude::*;
use scouter_types::agent::utils::AssertionTasks;
use scouter_types::agent::{EvalScenario, TasksFile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Collection of evaluation scenarios with their associated data and results.
///
/// `EvalScenarios` is the data model that holds:
/// - The scenario definitions (`Vec<EvalScenario>`)
/// - Internal state populated by `EvalRunner::collect_scenario_data()` (not serialized)
/// - Output populated by `EvalRunner::evaluate()` (serialized)
#[pyclass(from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScenarios {
    #[pyo3(get)]
    pub scenarios: Vec<EvalScenario>,

    #[pyo3(get)]
    #[serde(default = "create_uuid7")]
    pub collection_id: String,

    // Internal — not Python-visible, skipped in serde
    #[serde(skip)]
    pub(crate) scenario_datasets: HashMap<String, HashMap<String, EvalDataset>>,
    #[serde(skip)]
    pub(crate) scenario_contexts: HashMap<String, Value>,

    // Output — populated after evaluate()
    pub dataset_results: HashMap<String, EvalResults>,
    pub scenario_results: Vec<ScenarioResult>,
    #[pyo3(get)]
    pub metrics: Option<EvalMetrics>,
}

#[pymethods]
impl EvalScenarios {
    #[new]
    pub fn new(scenarios: Vec<EvalScenario>) -> Self {
        Self {
            scenarios,
            collection_id: create_uuid7(),
            scenario_datasets: HashMap::new(),
            scenario_contexts: HashMap::new(),
            dataset_results: HashMap::new(),
            scenario_results: Vec::new(),
            metrics: None,
        }
    }

    #[staticmethod]
    pub fn from_path(path: PathBuf) -> Result<Self, EvaluationError> {
        let ext = path.extension().and_then(|e| e.to_str()).ok_or_else(|| {
            EvaluationError::TypeError(scouter_types::error::TypeError::Error(format!(
                "Cannot determine file extension for: {}",
                path.display()
            )))
        })?;

        match ext.to_lowercase().as_str() {
            "jsonl" => load_from_jsonl(&path),
            "json" => load_from_json(&path),
            "yaml" | "yml" => load_from_yaml(&path),
            other => Err(EvaluationError::TypeError(
                scouter_types::error::TypeError::Error(format!(
                    "Unsupported extension '.{}'. Expected .jsonl, .json, .yaml, or .yml",
                    other
                )),
            )),
        }
    }

    #[getter]
    pub fn dataset_results(&self) -> HashMap<String, EvalResults> {
        self.dataset_results.clone()
    }

    #[getter]
    pub fn scenario_results(&self) -> Vec<ScenarioResult> {
        self.scenario_results.clone()
    }

    pub fn __len__(&self) -> usize {
        self.scenarios.len()
    }

    pub fn __bool__(&self) -> bool {
        !self.scenarios.is_empty()
    }

    pub fn is_evaluated(&self) -> bool {
        self.metrics.is_some()
    }

    pub fn model_dump_json(&self) -> Result<String, EvaluationError> {
        serde_json::to_string(self).map_err(Into::into)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<Self, EvaluationError> {
        serde_json::from_str(&json_string).map_err(Into::into)
    }

    // set uid
    #[setter]
    pub fn set_collection_id(&mut self, collection_id: String) {
        self.collection_id = collection_id;
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

// ---------------------------------------------------------------------------
// File loading helpers
// ---------------------------------------------------------------------------

fn default_max_turns_entry() -> usize {
    10
}

/// Intermediate deserialization type for scenario files using flat task lists.
/// Tasks are a plain array with a `task_type` discriminator, matching the
/// same format as `TasksFile`. The internal `EvalScenario` stores them in
/// bucketed `AssertionTasks`; this type bridges the two representations.
#[derive(Deserialize)]
struct ScenarioEntry {
    #[serde(default = "create_uuid7")]
    id: String,
    initial_query: String,
    #[serde(default)]
    predefined_turns: Vec<String>,
    simulated_user_persona: Option<String>,
    termination_signal: Option<String>,
    #[serde(default = "default_max_turns_entry")]
    max_turns: usize,
    expected_outcome: Option<String>,
    /// Flat task list — deserialized via `TasksFile`'s custom logic.
    #[serde(default)]
    tasks: Option<TasksFile>,
    metadata: Option<HashMap<String, Value>>,
}

impl From<ScenarioEntry> for EvalScenario {
    fn from(entry: ScenarioEntry) -> Self {
        let tasks = entry
            .tasks
            .map(AssertionTasks::from_tasks_file)
            .unwrap_or_else(|| AssertionTasks {
                assertion: vec![],
                judge: vec![],
                trace: vec![],
                agent: vec![],
            });
        EvalScenario {
            id: entry.id,
            initial_query: entry.initial_query,
            predefined_turns: entry.predefined_turns,
            simulated_user_persona: entry.simulated_user_persona,
            termination_signal: entry.termination_signal,
            max_turns: entry.max_turns,
            expected_outcome: entry.expected_outcome,
            tasks,
            metadata: entry.metadata,
        }
    }
}

/// Flat-task JSON/YAML file format supporting both a bare array and a wrapper
/// object with an optional `collection_id`.
#[derive(Deserialize)]
#[serde(untagged)]
enum ScenariosFileRaw {
    Wrapped {
        #[serde(default = "create_uuid7")]
        collection_id: String,
        scenarios: Vec<ScenarioEntry>,
    },
    Direct(Vec<ScenarioEntry>),
}

fn scenarios_from_raw(raw: ScenariosFileRaw) -> EvalScenarios {
    match raw {
        ScenariosFileRaw::Wrapped {
            collection_id,
            scenarios,
        } => {
            let mut s = EvalScenarios::new(scenarios.into_iter().map(EvalScenario::from).collect());
            s.collection_id = collection_id;
            s
        }
        ScenariosFileRaw::Direct(entries) => {
            EvalScenarios::new(entries.into_iter().map(EvalScenario::from).collect())
        }
    }
}

/// Load scenarios from a `.jsonl` file. One `EvalScenario` JSON object per non-empty line.
/// Note: a `collection_id` field in the file is not read — the collection ID is always
/// auto-generated. Use `.json` or `.yaml` with the wrapped format to preserve a specific ID.
fn load_from_jsonl(path: &PathBuf) -> Result<EvalScenarios, EvaluationError> {
    let content = std::fs::read_to_string(path)?;
    let scenarios: Vec<EvalScenario> = content
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, line)| {
            let entry: ScenarioEntry =
                serde_json::from_str(line).map_err(|e| EvaluationError::ParseLineError {
                    line: i + 1,
                    reason: e.to_string(),
                })?;
            Ok(EvalScenario::from(entry))
        })
        .collect::<Result<Vec<_>, EvaluationError>>()?;
    Ok(EvalScenarios::new(scenarios))
}

fn load_from_json(path: &PathBuf) -> Result<EvalScenarios, EvaluationError> {
    let content = std::fs::read_to_string(path)?;
    // Try bucketed format first (roundtrip from model_dump_json)
    if let Ok(s) = serde_json::from_str::<EvalScenarios>(&content) {
        return Ok(s);
    }
    // Fall back to flat-task format
    let raw: ScenariosFileRaw = serde_json::from_str(&content)?;
    Ok(scenarios_from_raw(raw))
}

fn load_from_yaml(path: &PathBuf) -> Result<EvalScenarios, EvaluationError> {
    let content = std::fs::read_to_string(path)?;
    // Try bucketed format first (roundtrip)
    if let Ok(s) = serde_yaml::from_str::<EvalScenarios>(&content) {
        return Ok(s);
    }
    // Fall back to flat-task format
    let raw: ScenariosFileRaw =
        serde_yaml::from_str(&content).map_err(scouter_types::error::TypeError::from)?;
    Ok(scenarios_from_raw(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::agent::utils::AssertionTasks;

    fn make_scenario(id: &str, query: &str) -> EvalScenario {
        EvalScenario {
            id: id.to_string(),
            initial_query: query.to_string(),
            predefined_turns: vec![],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 10,
            expected_outcome: None,
            tasks: AssertionTasks {
                assertion: vec![],
                judge: vec![],
                trace: vec![],
                agent: vec![],
            },
            metadata: None,
        }
    }

    #[test]
    fn construction_and_len() {
        let scenarios = EvalScenarios::new(vec![
            make_scenario("s1", "Hello"),
            make_scenario("s2", "World"),
        ]);
        assert_eq!(scenarios.__len__(), 2);
        assert!(!scenarios.is_evaluated());
    }

    #[test]
    fn is_evaluated_before_and_after() {
        let mut scenarios = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        assert!(!scenarios.is_evaluated());

        scenarios.metrics = Some(EvalMetrics {
            overall_pass_rate: 1.0,
            dataset_pass_rates: HashMap::new(),
            scenario_pass_rate: 1.0,
            workflow_pass_rate: 1.0,
            total_scenarios: 1,
            passed_scenarios: 1,
            scenario_task_pass_rates: HashMap::new(),
        });
        assert!(scenarios.is_evaluated());
    }

    #[test]
    fn is_empty_true_and_false() {
        let empty = EvalScenarios::new(vec![]);
        assert!(!empty.__bool__());

        let non_empty = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        assert!(non_empty.__bool__());
    }

    #[test]
    fn collection_id_auto_generated() {
        let s = EvalScenarios::new(vec![]);
        assert!(!s.collection_id.is_empty());
    }

    #[test]
    fn model_dump_json_roundtrip() {
        let scenarios = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        let original_id = scenarios.collection_id.clone();
        let json = scenarios.model_dump_json().unwrap();
        let loaded: EvalScenarios = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.scenarios.len(), 1);
        assert_eq!(loaded.scenarios[0].id, "s1");
        assert_eq!(loaded.collection_id, original_id);
        // serde-skipped fields should be empty
        assert!(loaded.scenario_datasets.is_empty());
        assert!(loaded.scenario_contexts.is_empty());
    }

    #[test]
    fn from_path_jsonl() {
        let jsonl = r#"{"id": "s1", "initial_query": "Hello?", "tasks": [{"task_type": "Assertion", "id": "check", "context_path": "response", "operator": "Contains", "expected_value": "hi"}]}
{"id": "s2", "initial_query": "Goodbye?"}"#;

        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.scenarios.len(), 2);
        assert_eq!(loaded.scenarios[0].id, "s1");
        assert_eq!(loaded.scenarios[1].id, "s2");
        assert_eq!(loaded.scenarios[0].tasks.assertion.len(), 1);
        assert!(!loaded.collection_id.is_empty());
    }

    #[test]
    fn from_path_json_array() {
        let json =
            r#"[{"id": "s1", "initial_query": "Hello?"}, {"id": "s2", "initial_query": "Bye?"}]"#;

        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios_array.json");
        std::fs::write(&path, json).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.scenarios.len(), 2);
    }

    #[test]
    fn from_path_json_wrapped_with_collection_id() {
        let collection_id = "my-custom-id";
        let json = format!(
            r#"{{"collection_id": "{}", "scenarios": [{{"id": "s1", "initial_query": "Hi?"}}]}}"#,
            collection_id
        );

        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios_wrapped.json");
        std::fs::write(&path, &json).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.collection_id, collection_id);
        assert_eq!(loaded.scenarios.len(), 1);
    }

    #[test]
    fn from_path_json_roundtrip() {
        let original = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        let original_cid = original.collection_id.clone();
        let json = original.model_dump_json().unwrap();

        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios_roundtrip.json");
        std::fs::write(&path, &json).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.collection_id, original_cid);
        assert_eq!(loaded.scenarios.len(), 1);
        assert_eq!(loaded.scenarios[0].id, "s1");
    }

    #[test]
    fn from_path_yaml_array() {
        let yaml = r#"
- id: s1
  initial_query: "Hello?"
- id: s2
  initial_query: "Bye?"
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios_array.yaml");
        std::fs::write(&path, yaml).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.scenarios.len(), 2);
        assert_eq!(loaded.scenarios[0].id, "s1");
        assert!(!loaded.collection_id.is_empty());
    }

    #[test]
    fn from_path_yaml_wrapped_with_collection_id() {
        let collection_id = "yaml-custom-id";
        let yaml = format!(
            "collection_id: {}\nscenarios:\n  - id: s1\n    initial_query: Hi?\n",
            collection_id
        );
        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios_wrapped.yaml");
        std::fs::write(&path, &yaml).unwrap();

        let loaded = EvalScenarios::from_path(path).unwrap();
        assert_eq!(loaded.collection_id, collection_id);
        assert_eq!(loaded.scenarios.len(), 1);
    }

    #[test]
    fn from_path_unsupported_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_scenarios.csv");
        std::fs::write(&path, "id,query\ns1,hello\n").unwrap();

        let err = EvalScenarios::from_path(path).unwrap_err();
        assert!(
            matches!(err, EvaluationError::TypeError(_)),
            "expected TypeError, got: {err:?}"
        );
    }

    #[test]
    fn from_path_nonexistent_file() {
        let path = PathBuf::from("/tmp/does_not_exist_abc123.jsonl");
        let err = EvalScenarios::from_path(path).unwrap_err();
        assert!(
            matches!(err, EvaluationError::IoError(_)),
            "expected IoError, got: {err:?}"
        );
    }

    #[test]
    fn from_path_malformed_jsonl_reports_line_number() {
        let jsonl = "{\"id\": \"s1\", \"initial_query\": \"ok\"}\n{bad json on line 2}";
        let dir = std::env::temp_dir();
        let path = dir.join("test_malformed.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let err = EvalScenarios::from_path(path).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("line 2"),
            "expected error to mention 'line 2', got: {msg}"
        );
    }
}
