use crate::error::EvaluationError;
use crate::evaluate::evaluator::GenAIEvaluator;
use crate::evaluate::scenario_results::{EvalMetrics, ScenarioEvalResults, ScenarioResult};
use crate::evaluate::types::{EvalResults, EvaluationConfig};
use crate::genai::{evaluate_genai_dataset, EvalDataset};
use crate::scenario::EvalScenarios;
use pyo3::prelude::*;
use scouter_state::app_state;
use scouter_types::genai::{GenAIEvalConfig, GenAIEvalProfile};
use scouter_types::trace::build_trace_spans;
use scouter_types::EvalRecord;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, error};

type AliasData = (
    Vec<EvalRecord>,
    Option<Arc<GenAIEvalProfile>>,
    Vec<scouter_types::trace::sql::TraceSpan>,
);

/// Stateful evaluation engine that orchestrates scenario evaluation.
///
/// `EvalRunner` owns the scenario definitions and profiles (as `Arc`s),
/// mirroring the `ScouterQueue` pattern. It provides:
/// - `add_scenario_data()`: Populates scenario datasets and contexts
/// - `evaluate()`: Runs multi-level evaluation (sub-agent + scenario + aggregate),
///   pulling captured spans from the global buffer automatically.
#[pyclass]
#[derive(Debug)]
pub struct EvalRunner {
    profiles: HashMap<String, Arc<GenAIEvalProfile>>,
    scenarios: EvalScenarios,
}

#[pymethods]
impl EvalRunner {
    #[new]
    #[pyo3(signature = (scenarios, profiles))]
    pub fn new(
        scenarios: Vec<scouter_types::genai::EvalScenario>,
        profiles: HashMap<String, GenAIEvalProfile>,
    ) -> Self {
        let arc_profiles: HashMap<String, Arc<GenAIEvalProfile>> = profiles
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();
        Self {
            profiles: arc_profiles,
            scenarios: EvalScenarios::new(scenarios),
        }
    }

    #[getter]
    pub fn scenarios(&self) -> EvalScenarios {
        self.scenarios.clone()
    }

    /// Run multi-level evaluation.
    ///
    /// Spans are pulled automatically from the global capture buffer —
    /// no need to pass them explicitly.
    ///
    /// **LEVEL 1** — Sub-agent evaluation: flatten all records per alias → one EvalDataset → evaluate
    /// **LEVEL 2** — Scenario evaluation: per scenario with tasks, evaluate against response + traces
    /// **LEVEL 3** — Aggregate metrics
    #[pyo3(signature = (config=None))]
    pub fn evaluate(
        &mut self,
        config: Option<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        let config = Arc::new(config.unwrap_or_default());

        app_state()
            .handle()
            .block_on(async { self.evaluate_async(&config).await })
    }
}

impl EvalRunner {
    /// Populate scenario data into the internal `EvalScenarios` container.
    ///
    /// # Arguments
    /// * `scenario_id` - ID of the scenario being populated
    /// * `records` - Map of alias → eval records for this scenario
    /// * `response` - The agent's final response for this scenario
    /// * `scenario` - Reference to the scenario definition
    pub fn add_scenario_data(
        &mut self,
        records: HashMap<String, Vec<EvalRecord>>,
        response: String,
        scenario: &scouter_types::genai::EvalScenario,
    ) -> Result<(), EvaluationError> {
        let mut alias_datasets: HashMap<String, EvalDataset> = HashMap::new();
        let scenario_id = scenario.id.clone();

        for (alias, mut alias_records) in records {
            // Tag each record with the scenario_id
            for record in &mut alias_records {
                record.tag = Some(scenario_id.clone());
            }

            let profile = self.profiles.get(&alias).ok_or_else(|| {
                EvaluationError::MissingKeyError(format!(
                    "No profile found for alias '{}' in scenario '{}'",
                    alias, scenario_id
                ))
            })?;

            alias_datasets.insert(
                alias,
                EvalDataset {
                    records: Arc::new(alias_records),
                    profile: Arc::clone(profile),
                    spans: Arc::new(vec![]),
                },
            );
        }

        self.scenarios
            .scenario_datasets
            .insert(scenario_id.clone(), alias_datasets);

        // Build context JSON for scenario-level evaluation
        let context = json!({
            "response": response,
            "expected_outcome": scenario.expected_outcome,
            "metadata": scenario.metadata,
        });
        self.scenarios
            .scenario_contexts
            .insert(scenario_id, context);

        Ok(())
    }

    async fn evaluate_async(
        &mut self,
        config: &Arc<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        // Collect all trace_ids from stored records (one pass)
        let all_trace_ids = self.collect_all_trace_ids();

        // Single targeted query — only clones matching spans
        let captured_spans = if !all_trace_ids.is_empty() {
            scouter_types::span_capture::get_captured_spans_by_trace_ids(&all_trace_ids)
        } else {
            vec![]
        };

        // Convert once, use for both levels
        let trace_spans = build_trace_spans(captured_spans);

        // ── LEVEL 1: Sub-agent evaluation ──
        // Set spans on alias datasets from pre-converted TraceSpans
        self.set_dataset_spans(&trace_spans);
        let dataset_results = self.evaluate_datasets(config).await?;

        // ── LEVEL 2: Scenario evaluation ──
        let scenario_results = self.evaluate_scenarios(&trace_spans).await?;

        // ── LEVEL 3: Aggregate metrics ──
        let metrics = compute_metrics(&dataset_results, &scenario_results);

        // Store results on the EvalScenarios container
        self.scenarios.dataset_results = dataset_results.clone();
        self.scenarios.scenario_results = scenario_results.clone();
        self.scenarios.metrics = Some(metrics.clone());

        Ok(ScenarioEvalResults {
            dataset_results,
            scenario_results,
            metrics,
        })
    }

    /// Set spans on each alias dataset by filtering pre-converted TraceSpans to matching trace_ids.
    fn set_dataset_spans(&mut self, trace_spans: &[scouter_types::trace::sql::TraceSpan]) {
        for datasets in self.scenarios.scenario_datasets.values_mut() {
            for dataset in datasets.values_mut() {
                let trace_ids: HashSet<String> = dataset
                    .records
                    .iter()
                    .filter_map(|r| r.trace_id.as_ref().map(|tid| tid.to_hex()))
                    .collect();

                if trace_ids.is_empty() {
                    continue;
                }

                let matching: Vec<_> = trace_spans
                    .iter()
                    .filter(|s| trace_ids.contains(&s.trace_id))
                    .cloned()
                    .collect();

                if !matching.is_empty() {
                    dataset.spans = Arc::new(matching);
                }
            }
        }
    }

    /// Collect all trace_ids from records across all scenario datasets.
    fn collect_all_trace_ids(&self) -> HashSet<scouter_types::TraceId> {
        self.scenarios
            .scenario_datasets
            .values()
            .flat_map(|datasets| datasets.values())
            .flat_map(|dataset| dataset.records.iter())
            .filter_map(|r| r.trace_id)
            .collect()
    }

    /// LEVEL 1: For each alias across all scenarios, flatten records into one
    /// dataset and evaluate holistically.
    async fn evaluate_datasets(
        &self,
        config: &Arc<EvaluationConfig>,
    ) -> Result<HashMap<String, EvalResults>, EvaluationError> {
        // Collect all aliases
        let mut alias_data: HashMap<String, AliasData> = HashMap::new();

        for datasets in self.scenarios.scenario_datasets.values() {
            for (alias, dataset) in datasets {
                let entry = alias_data
                    .entry(alias.clone())
                    .or_insert_with(|| (Vec::new(), None, Vec::new()));
                entry.0.extend(dataset.records.iter().cloned());
                if entry.1.is_none() {
                    entry.1 = Some(Arc::clone(&dataset.profile));
                }
                entry.2.extend(dataset.spans.iter().cloned());
            }
        }

        let mut results = HashMap::new();

        for (alias, (records, profile, spans)) in alias_data {
            if records.is_empty() {
                continue;
            }

            let profile = match profile {
                Some(p) => p,
                None => continue,
            };

            let dataset = EvalDataset {
                records: Arc::new(records),
                profile,
                spans: Arc::new(spans),
            };

            debug!("Evaluating sub-agent dataset for alias '{}'", alias);
            match evaluate_genai_dataset(&dataset, config).await {
                Ok(eval_results) => {
                    results.insert(alias, eval_results);
                }
                Err(e) => {
                    error!("Failed to evaluate dataset for alias '{}': {:?}", alias, e);
                    return Err(e);
                }
            }
        }

        Ok(results)
    }

    /// LEVEL 2: For each scenario that has tasks, build a record from
    /// the scenario context and evaluate against the profile + filtered traces.
    async fn evaluate_scenarios(
        &self,
        trace_spans: &[scouter_types::trace::sql::TraceSpan],
    ) -> Result<Vec<ScenarioResult>, EvaluationError> {
        let mut results = Vec::new();

        // Collect all trace_ids per scenario (from records)
        let scenario_trace_ids = self.collect_scenario_trace_ids();

        for scenario in &self.scenarios.scenarios {
            if !scenario.has_tasks() {
                continue;
            }

            // Fix 7: Error on missing context instead of silently skipping
            let context = self
                .scenarios
                .scenario_contexts
                .get(&scenario.id)
                .cloned()
                .ok_or_else(|| {
                    EvaluationError::MissingKeyError(format!(
                        "Scenario '{}' has tasks but no context — call add_scenario_data() first",
                        scenario.id
                    ))
                })?;

            // Build EvalRecord from scenario context
            let record = EvalRecord {
                context,
                record_id: scenario.id.clone(),
                tag: Some(scenario.id.clone()),
                ..Default::default()
            };

            // Build profile from scenario tasks
            let profile = GenAIEvalProfile::build_from_parts_async(
                GenAIEvalConfig::default(),
                scenario.tasks.clone(),
                None,
            )
            .await?;
            let profile = Arc::new(profile);

            // Filter trace spans for this scenario's trace_ids
            let filtered_spans = if let Some(trace_ids) = scenario_trace_ids.get(&scenario.id) {
                trace_spans
                    .iter()
                    .filter(|s| trace_ids.contains(&s.trace_id))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let spans_arc = Arc::new(filtered_spans);

            // Evaluate
            match GenAIEvaluator::process_event_record(&record, profile, spans_arc).await {
                Ok(eval_set) => {
                    let mut eval_results = EvalResults::new();
                    eval_results.add_success(&record, eval_set, BTreeMap::new());

                    let (passed, pass_rate) = compute_pass_rate(&eval_results);

                    results.push(ScenarioResult {
                        scenario_id: scenario.id.clone(),
                        initial_query: scenario.initial_query.clone(),
                        eval_results,
                        passed,
                        pass_rate,
                    });
                }
                Err(e) => {
                    error!("Failed to evaluate scenario '{}': {:?}", scenario.id, e);
                    let mut eval_results = EvalResults::new();
                    eval_results.add_failure(&record, e.to_string());

                    results.push(ScenarioResult {
                        scenario_id: scenario.id.clone(),
                        initial_query: scenario.initial_query.clone(),
                        eval_results,
                        passed: false,
                        pass_rate: 0.0,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Collect trace_ids from records for each scenario
    fn collect_scenario_trace_ids(&self) -> HashMap<String, HashSet<String>> {
        let mut result: HashMap<String, HashSet<String>> = HashMap::new();

        for (scenario_id, datasets) in &self.scenarios.scenario_datasets {
            let mut trace_ids = HashSet::new();
            for dataset in datasets.values() {
                for record in dataset.records.iter() {
                    if let Some(ref tid) = record.trace_id {
                        trace_ids.insert(tid.to_hex());
                    }
                }
            }
            if !trace_ids.is_empty() {
                result.insert(scenario_id.clone(), trace_ids);
            }
        }

        result
    }
}

/// LEVEL 3: Compute aggregate metrics (Fix 9: no unnecessary collect)
fn compute_metrics(
    dataset_results: &HashMap<String, EvalResults>,
    scenario_results: &[ScenarioResult],
) -> EvalMetrics {
    // Dataset pass rates
    let mut dataset_pass_rates: HashMap<String, f64> = HashMap::new();
    for (alias, results) in dataset_results {
        let (_, pass_rate) = compute_pass_rate(results);
        dataset_pass_rates.insert(alias.clone(), pass_rate);
    }

    // Scenario pass rate
    let total_scenarios = scenario_results.len();
    let passed_scenarios = scenario_results.iter().filter(|s| s.passed).count();
    let scenario_pass_rate = if total_scenarios > 0 {
        passed_scenarios as f64 / total_scenarios as f64
    } else {
        0.0
    };

    // Overall pass rate: weighted average of dataset pass rates and scenario pass rate
    let mut all_rates: Vec<f64> = dataset_pass_rates.values().copied().collect();
    if total_scenarios > 0 {
        all_rates.push(scenario_pass_rate);
    }
    let overall_pass_rate = if all_rates.is_empty() {
        0.0
    } else {
        all_rates.iter().sum::<f64>() / all_rates.len() as f64
    };

    EvalMetrics {
        overall_pass_rate,
        dataset_pass_rates,
        scenario_pass_rate,
        total_scenarios,
        passed_scenarios,
    }
}

/// Compute pass/fail and pass rate from EvalResults
fn compute_pass_rate(results: &EvalResults) -> (bool, f64) {
    if results.aligned_results.is_empty() {
        return (true, 1.0);
    }

    let mut total_tasks = 0;
    let mut passed_tasks = 0;

    for aligned in &results.aligned_results {
        for task_result in &aligned.eval_set.records {
            total_tasks += 1;
            if task_result.passed {
                passed_tasks += 1;
            }
        }
    }

    // Fix 6: aligned_results exist but contain zero task records → false positive
    if total_tasks == 0 {
        return (false, 0.0);
    }

    let pass_rate = passed_tasks as f64 / total_tasks as f64;
    let passed = passed_tasks == total_tasks;
    (passed, pass_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::genai::utils::AssertionTasks;
    use scouter_types::genai::EvalScenario;

    fn empty_tasks() -> AssertionTasks {
        AssertionTasks {
            assertion: vec![],
            judge: vec![],
            trace: vec![],
            agent: vec![],
        }
    }

    fn make_scenario(id: &str, query: &str) -> EvalScenario {
        EvalScenario {
            id: id.to_string(),
            initial_query: query.to_string(),
            predefined_turns: vec![],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 10,
            expected_outcome: Some("Expected output".to_string()),
            tasks: empty_tasks(),
            metadata: None,
        }
    }

    fn make_scenario_with_tasks(id: &str, query: &str) -> EvalScenario {
        use scouter_types::genai::{AssertionTask, ComparisonOperator, EvaluationTaskType};

        let task = AssertionTask {
            id: "check_response".to_string(),
            context_path: Some("response".to_string()),
            item_context_path: None,
            operator: ComparisonOperator::Contains,
            expected_value: serde_json::Value::String("hello".to_string()),
            description: None,
            depends_on: vec![],
            task_type: EvaluationTaskType::Assertion,
            result: None,
            condition: false,
        };

        EvalScenario {
            id: id.to_string(),
            initial_query: query.to_string(),
            predefined_turns: vec![],
            simulated_user_persona: None,
            termination_signal: None,
            max_turns: 10,
            expected_outcome: Some("Response contains hello".to_string()),
            tasks: AssertionTasks {
                assertion: vec![task],
                judge: vec![],
                trace: vec![],
                agent: vec![],
            },
            metadata: None,
        }
    }

    fn make_default_profiles() -> HashMap<String, GenAIEvalProfile> {
        let mut profiles = HashMap::new();
        profiles.insert("agent_a".to_string(), GenAIEvalProfile::default());
        profiles
    }

    #[test]
    fn add_scenario_data_stores_datasets_and_contexts() {
        let mut runner =
            EvalRunner::new(vec![make_scenario("s1", "Hello")], make_default_profiles());

        let mut records = HashMap::new();
        let record = EvalRecord::default();
        records.insert("agent_a".to_string(), vec![record]);

        let scenario = runner.scenarios.scenarios[0].clone();

        runner
            .add_scenario_data(records, "Agent response".to_string(), &scenario)
            .unwrap();

        // Verify datasets stored
        assert!(runner.scenarios.scenario_datasets.contains_key("s1"));
        let datasets = &runner.scenarios.scenario_datasets["s1"];
        assert!(datasets.contains_key("agent_a"));
        assert_eq!(datasets["agent_a"].records.len(), 1);

        // Verify records are tagged
        assert_eq!(datasets["agent_a"].records[0].tag, Some("s1".to_string()));

        // Verify context stored
        assert!(runner.scenarios.scenario_contexts.contains_key("s1"));
        let ctx = &runner.scenarios.scenario_contexts["s1"];
        assert_eq!(ctx["response"], "Agent response");
    }

    #[test]
    fn add_scenario_data_missing_profile_errors() {
        let mut runner = EvalRunner::new(
            vec![make_scenario("s1", "Hello")],
            HashMap::new(), // No profiles
        );

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);

        let scenario = runner.scenarios.scenarios[0].clone();

        let result = runner.add_scenario_data(records, "Response".to_string(), &scenario);

        assert!(result.is_err());
    }

    #[test]
    fn add_scenario_data_multiple_aliases() {
        let mut profiles = make_default_profiles();
        profiles.insert("agent_b".to_string(), GenAIEvalProfile::default());

        let mut runner = EvalRunner::new(vec![make_scenario("s1", "Hello")], profiles);

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);
        records.insert(
            "agent_b".to_string(),
            vec![EvalRecord::default(), EvalRecord::default()],
        );

        let scenario = runner.scenarios.scenarios[0].clone();
        runner
            .add_scenario_data(records, "Response".to_string(), &scenario)
            .unwrap();

        let datasets = &runner.scenarios.scenario_datasets["s1"];
        assert_eq!(datasets["agent_a"].records.len(), 1);
        assert_eq!(datasets["agent_b"].records.len(), 2);
    }

    #[test]
    fn evaluate_no_tasks_only_datasets() {
        let mut runner =
            EvalRunner::new(vec![make_scenario("s1", "Hello")], make_default_profiles());

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);

        let scenario = runner.scenarios.scenarios[0].clone();
        runner
            .add_scenario_data(records, "Response".to_string(), &scenario)
            .unwrap();

        // Evaluate
        let result = runner.evaluate(None).unwrap();

        // Should have dataset results for agent_a
        assert!(result.dataset_results.contains_key("agent_a"));

        // No scenario results (no tasks on scenarios)
        assert!(result.scenario_results.is_empty());

        // Metrics should be computed
        assert!(result.metrics.dataset_pass_rates.contains_key("agent_a"));
        assert_eq!(result.metrics.total_scenarios, 0);
    }

    #[test]
    fn evaluate_with_assertion_tasks() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner = EvalRunner::new(vec![scenario.clone()], HashMap::new());

        // Add scenario context (simulate add_scenario_data for context only)
        let context = json!({
            "response": "hello world",
            "expected_outcome": "Response contains hello",
            "metadata": null,
        });
        runner
            .scenarios
            .scenario_contexts
            .insert("s1".to_string(), context);

        // Evaluate — scenario has an assertion task checking response contains "hello"
        let result = runner.evaluate(None).unwrap();

        // Should have scenario results
        assert_eq!(result.scenario_results.len(), 1);
        assert_eq!(result.scenario_results[0].scenario_id, "s1");
        assert!(result.scenario_results[0].passed);
        assert_eq!(result.scenario_results[0].pass_rate, 1.0);

        // Metrics
        assert_eq!(result.metrics.total_scenarios, 1);
        assert_eq!(result.metrics.passed_scenarios, 1);
        assert_eq!(result.metrics.scenario_pass_rate, 1.0);
    }

    #[test]
    fn evaluate_with_failing_assertion() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner = EvalRunner::new(vec![scenario.clone()], HashMap::new());

        // Context where response does NOT contain "hello"
        let context = json!({
            "response": "goodbye world",
            "expected_outcome": "Response contains hello",
            "metadata": null,
        });
        runner
            .scenarios
            .scenario_contexts
            .insert("s1".to_string(), context);

        let result = runner.evaluate(None).unwrap();

        assert_eq!(result.scenario_results.len(), 1);
        assert!(!result.scenario_results[0].passed);
        assert_eq!(result.scenario_results[0].pass_rate, 0.0);
        assert_eq!(result.metrics.passed_scenarios, 0);
    }

    #[test]
    fn evaluate_scenario_with_tasks_but_no_context_errors() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner = EvalRunner::new(vec![scenario], HashMap::new());

        // Don't add any context — should error
        let result = runner.evaluate(None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("no context"));
    }

    #[test]
    fn compute_pass_rate_empty_results() {
        let results = EvalResults::new();
        let (passed, rate) = compute_pass_rate(&results);
        assert!(passed);
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn compute_pass_rate_zero_tasks() {
        // aligned_results exist but contain zero task records
        let mut results = EvalResults::new();
        let record = EvalRecord::default();
        let eval_set = scouter_types::genai::EvalSet::new(
            vec![], // No task results
            Default::default(),
        );
        results.add_success(&record, eval_set, BTreeMap::new());

        let (passed, rate) = compute_pass_rate(&results);
        assert!(!passed); // Fix 6: should be false, not true
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn evaluate_multiple_scenarios_mixed_results() {
        let s_pass = make_scenario_with_tasks("s_pass", "Say hello");
        let s_fail = make_scenario_with_tasks("s_fail", "Say hello");
        let mut runner = EvalRunner::new(vec![s_pass, s_fail], HashMap::new());

        // Passing context
        runner.scenarios.scenario_contexts.insert(
            "s_pass".to_string(),
            json!({"response": "hello world", "expected_outcome": null, "metadata": null}),
        );
        // Failing context
        runner.scenarios.scenario_contexts.insert(
            "s_fail".to_string(),
            json!({"response": "goodbye", "expected_outcome": null, "metadata": null}),
        );

        let result = runner.evaluate(None).unwrap();
        assert_eq!(result.scenario_results.len(), 2);
        assert_eq!(result.metrics.total_scenarios, 2);
        assert_eq!(result.metrics.passed_scenarios, 1);
        assert_eq!(result.metrics.scenario_pass_rate, 0.5);
    }

    #[test]
    fn compute_metrics_empty() {
        let metrics = compute_metrics(&HashMap::new(), &[]);

        assert_eq!(metrics.overall_pass_rate, 0.0);
        assert_eq!(metrics.scenario_pass_rate, 0.0);
        assert_eq!(metrics.total_scenarios, 0);
        assert_eq!(metrics.passed_scenarios, 0);
    }

    #[test]
    fn compute_metrics_with_scenario_results() {
        let scenario_results = vec![
            ScenarioResult {
                scenario_id: "s1".to_string(),
                initial_query: "Q1".to_string(),
                eval_results: EvalResults::new(),
                passed: true,
                pass_rate: 1.0,
            },
            ScenarioResult {
                scenario_id: "s2".to_string(),
                initial_query: "Q2".to_string(),
                eval_results: EvalResults::new(),
                passed: false,
                pass_rate: 0.5,
            },
        ];

        let metrics = compute_metrics(&HashMap::new(), &scenario_results);

        assert_eq!(metrics.total_scenarios, 2);
        assert_eq!(metrics.passed_scenarios, 1);
        assert_eq!(metrics.scenario_pass_rate, 0.5);
        // Only scenario pass rate contributes
        assert_eq!(metrics.overall_pass_rate, 0.5);
    }
}
