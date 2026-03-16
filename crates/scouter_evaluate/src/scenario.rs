use crate::error::EvaluationError;
use crate::evaluate::scenario_results::{EvalMetrics, ScenarioResult};
use crate::evaluate::types::EvalResults;
use crate::genai::EvalDataset;
use potato_head::PyHelperFuncs;
use pyo3::prelude::*;
use scouter_types::genai::EvalScenario;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Collection of evaluation scenarios with their associated data and results.
///
/// `EvalScenarios` is the data model that holds:
/// - The scenario definitions (`Vec<EvalScenario>`)
/// - Internal state populated by `EvalRunner::collect_scenario_data()` (not serialized)
/// - Output populated by `EvalRunner::evaluate()` (serialized)
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScenarios {
    #[pyo3(get)]
    pub scenarios: Vec<EvalScenario>,

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
            scenario_datasets: HashMap::new(),
            scenario_contexts: HashMap::new(),
            dataset_results: HashMap::new(),
            scenario_results: Vec::new(),
            metrics: None,
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

    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
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

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::genai::utils::AssertionTasks;

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
        assert_eq!(scenarios.len(), 2);
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
            total_scenarios: 1,
            passed_scenarios: 1,
        });
        assert!(scenarios.is_evaluated());
    }

    #[test]
    fn is_empty_true_and_false() {
        let empty = EvalScenarios::new(vec![]);
        assert!(empty.is_empty());

        let non_empty = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn model_dump_json_roundtrip() {
        let scenarios = EvalScenarios::new(vec![make_scenario("s1", "Hello")]);
        let json = scenarios.model_dump_json().unwrap();
        let loaded: EvalScenarios = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.scenarios.len(), 1);
        assert_eq!(loaded.scenarios[0].id, "s1");
        // serde-skipped fields should be empty
        assert!(loaded.scenario_datasets.is_empty());
        assert!(loaded.scenario_contexts.is_empty());
    }
}
