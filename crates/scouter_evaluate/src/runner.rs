use crate::agent::{evaluate_genai_dataset, EvalDataset};
use crate::error::EvaluationError;
use crate::evaluate::evaluator::GenAIEvaluator;
use crate::evaluate::scenario_results::{
    EvalMetrics, ScenarioEvalResults, ScenarioResult, TaskSummary,
};
use crate::evaluate::types::{EvalResults, EvaluationConfig};
use crate::scenario::EvalScenarios;
use pyo3::prelude::*;
use scouter_state::app_state;
use scouter_types::agent::EvalScenario;
use scouter_types::agent::{AgentEvalConfig, AgentEvalProfile};
use scouter_types::trace::build_trace_spans;
use scouter_types::trace::sql::TraceSpan;
use scouter_types::EvalRecord;
use scouter_types::TraceId as ScouterTraceId;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, error};

struct AliasData {
    records: Vec<EvalRecord>,
    profile: Option<Arc<AgentEvalProfile>>,
    spans: Vec<TraceSpan>,
}

/// Stateful evaluation engine that orchestrates scenario evaluation.
///
/// `EvalRunner` owns the scenario definitions and profiles (as `Arc`s),
/// mirroring the `ScouterQueue` pattern. It provides:
/// - `collect_scenario_data()`: Populates scenario datasets and contexts
/// - `evaluate()`: Runs multi-level evaluation (sub-agent + scenario + aggregate),
///   pulling captured spans from the global buffer automatically.
#[derive(Debug)]
#[pyclass]
pub struct EvalRunner {
    profiles: HashMap<String, Arc<AgentEvalProfile>>,
    scenarios: EvalScenarios,
}

#[pymethods]
impl EvalRunner {
    #[new]
    #[pyo3(signature = (scenarios, profiles))]
    pub fn new(scenarios: EvalScenarios, profiles: HashMap<String, AgentEvalProfile>) -> Self {
        let arc_profiles: HashMap<String, Arc<AgentEvalProfile>> = profiles
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();
        Self {
            profiles: arc_profiles,
            scenarios,
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

        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(EvaluationError::GenAIEvaluatorError(
                "EvalRunner.evaluate() cannot be called from within an async context. \
                 Use evaluate_async() or call from a synchronous Python context."
                    .to_string(),
            ));
        }

        app_state()
            .handle()
            .block_on(async { self.evaluate_async(&config).await })
    }

    /// Populate scenario data into the internal `EvalScenarios` container.
    ///
    /// # Arguments
    /// * `records` - Map of alias → eval records for this scenario
    /// * `response` - The agent's final response for this scenario
    /// * `scenario` - Reference to the scenario definition
    #[pyo3(signature = (records, response, scenario))]
    pub fn collect_scenario_data(
        &mut self,
        records: HashMap<String, Vec<EvalRecord>>,
        response: String,
        scenario: &EvalScenario,
    ) -> Result<(), EvaluationError> {
        let mut alias_datasets: HashMap<String, EvalDataset> = HashMap::new();
        let scenario_id = scenario.id.clone();

        for (alias, mut alias_records) in records {
            // Tag each record with the scenario_id
            let scenario_tag = format!("scouter.eval.scenario_id={}", scenario_id);
            for record in &mut alias_records {
                if !record.tags.contains(&scenario_tag) {
                    record.tags.push(scenario_tag.clone());
                }
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

        if self.scenarios.scenario_datasets.contains_key(&scenario_id) {
            return Err(EvaluationError::MissingKeyError(format!(
                "Scenario '{}' already has data — collect_scenario_data called twice",
                scenario_id
            )));
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
}

impl EvalRunner {
    async fn evaluate_async(
        &mut self,
        config: &Arc<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        let scenario_ids: HashSet<String> = self
            .scenarios
            .scenarios
            .iter()
            .map(|s| s.id.clone())
            .collect();

        // Single buffer scan → grouped by scenario_id (via span attribute)
        let mut raw_by_scenario =
            scouter_types::span_capture::get_spans_grouped_by_scenario_id(&scenario_ids);

        // Also resolve spans via EvalRecord.trace_id for scenarios whose spans were not
        // captured with the scouter.eval.scenario_id attribute (e.g. plain OTel spans).
        let mut record_trace_to_scenario: HashMap<ScouterTraceId, String> = HashMap::new();
        for (scenario_id, datasets) in &self.scenarios.scenario_datasets {
            for dataset in datasets.values() {
                for record in dataset.records.iter() {
                    if let Some(tid) = record.trace_id {
                        record_trace_to_scenario.insert(tid, scenario_id.clone());
                    }
                }
            }
        }

        if !record_trace_to_scenario.is_empty() {
            // Only fetch trace_ids not already covered by the attribute-based pass
            let already_covered: HashSet<ScouterTraceId> = raw_by_scenario
                .values()
                .flat_map(|spans: &Vec<_>| spans.iter().map(|s| s.trace_id))
                .collect();
            let new_trace_ids: HashSet<ScouterTraceId> = record_trace_to_scenario
                .keys()
                .filter(|tid| !already_covered.contains(tid))
                .copied()
                .collect();
            if !new_trace_ids.is_empty() {
                let extra =
                    scouter_types::span_capture::get_captured_spans_by_trace_ids(&new_trace_ids);
                for span in extra {
                    if let Some(sid) = record_trace_to_scenario.get(&span.trace_id) {
                        raw_by_scenario.entry(sid.clone()).or_default().push(span);
                    }
                }
            }
        }

        // Convert each group to tree-enriched TraceSpans
        let spans_by_scenario: HashMap<String, Arc<Vec<TraceSpan>>> = raw_by_scenario
            .into_iter()
            .map(|(id, records)| (id, Arc::new(build_trace_spans(records))))
            .collect();

        // Assign spans to datasets by scenario_id key lookup
        for (scenario_id, datasets) in &mut self.scenarios.scenario_datasets {
            if let Some(spans) = spans_by_scenario.get(scenario_id) {
                for dataset in datasets.values_mut() {
                    dataset.spans = Arc::clone(spans);
                }
            }
        }

        // Level 1: Sub-agent evaluation
        let dataset_results = self.evaluate_datasets(config).await?;

        // Level 2: Scenario evaluation
        let scenario_results = self.evaluate_scenarios(&spans_by_scenario).await?;

        // Level 3: Aggregate metrics
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
                    .or_insert_with(|| AliasData {
                        records: Vec::new(),
                        profile: None,
                        spans: Vec::new(),
                    });
                entry.records.extend(dataset.records.iter().cloned());
                if entry.profile.is_none() {
                    entry.profile = Some(Arc::clone(&dataset.profile));
                } else {
                    // First-seen profile wins per alias — warn when a subsequent scenario
                    // provides a different profile instance for the same alias.
                    debug!(
                        "Alias '{}': profile already set — first-seen profile wins; ignoring profile from a subsequent scenario. Ensure all scenarios use the same profile for this alias.",
                        alias
                    );
                }
                entry.spans.extend(dataset.spans.iter().cloned());
            }
        }

        let mut results = HashMap::new();

        for (
            alias,
            AliasData {
                records,
                profile,
                spans,
            },
        ) in alias_data
        {
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
    /// the scenario context and evaluate against the profile + spans looked up by scenario_id.
    async fn evaluate_scenarios(
        &self,
        spans_by_scenario: &HashMap<String, Arc<Vec<TraceSpan>>>,
    ) -> Result<Vec<ScenarioResult>, EvaluationError> {
        let mut results = Vec::new();

        for scenario in &self.scenarios.scenarios {
            if !scenario.has_tasks() {
                continue;
            }

            let context = self
                .scenarios
                .scenario_contexts
                .get(&scenario.id)
                .cloned()
                .ok_or_else(|| {
                    EvaluationError::MissingKeyError(format!(
                        "Scenario '{}' has tasks but no context — call collect_scenario_data() first",
                        scenario.id
                    ))
                })?;

            // Build EvalRecord from scenario context
            let record = EvalRecord {
                context,
                record_id: scenario.id.clone(),
                tags: vec![format!("scouter.eval.scenario_id={}", scenario.id)],
                ..Default::default()
            };

            // Build profile from scenario tasks
            let profile = AgentEvalProfile::build_from_parts_async(
                AgentEvalConfig::default(),
                scenario.tasks.clone(),
                None,
            )
            .await?;
            let profile = Arc::new(profile);

            // Look up spans directly by scenario_id — no filtering needed
            let spans_arc = spans_by_scenario
                .get(&scenario.id)
                .cloned()
                .unwrap_or_else(|| Arc::new(Vec::new()));

            // Evaluate
            match GenAIEvaluator::process_event_record(&record, profile, spans_arc).await {
                Ok(eval_set) => {
                    let task_results: Vec<TaskSummary> = eval_set
                        .records
                        .iter()
                        .map(|r| TaskSummary {
                            task_id: r.task_id.clone(),
                            passed: r.passed,
                            value: if r.passed { 1.0 } else { 0.0 },
                        })
                        .collect();

                    let mut eval_results = EvalResults::new();
                    eval_results.add_success(&record, eval_set, BTreeMap::new());

                    let (passed, pass_rate) = compute_pass_rate(&eval_results);

                    results.push(ScenarioResult {
                        scenario_id: scenario.id.clone(),
                        initial_query: scenario.initial_query.clone(),
                        eval_results,
                        passed,
                        pass_rate,
                        task_results,
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
                        task_results: vec![],
                    });
                }
            }
        }

        Ok(results)
    }
}

/// LEVEL 3: Compute aggregate metrics
fn compute_metrics(
    dataset_results: &HashMap<String, EvalResults>,
    scenario_results: &[ScenarioResult],
) -> EvalMetrics {
    let mut dataset_pass_rates: HashMap<String, f64> = HashMap::new();
    for (alias, results) in dataset_results {
        let (_, pass_rate) = compute_pass_rate(results);
        dataset_pass_rates.insert(alias.clone(), pass_rate);
    }

    let total_scenarios = scenario_results.len();
    let passed_scenarios = scenario_results.iter().filter(|s| s.passed).count();
    let scenario_pass_rate = if total_scenarios > 0 {
        passed_scenarios as f64 / total_scenarios as f64
    } else {
        0.0
    };

    let workflow_rates: Vec<f64> = dataset_pass_rates.values().copied().collect();
    let workflow_pass_rate = if workflow_rates.is_empty() {
        0.0
    } else {
        workflow_rates.iter().sum::<f64>() / workflow_rates.len() as f64
    };

    let mut all_rates = workflow_rates.clone();
    if total_scenarios > 0 {
        all_rates.push(scenario_pass_rate);
    }
    let overall_pass_rate = if all_rates.is_empty() {
        0.0
    } else {
        all_rates.iter().sum::<f64>() / all_rates.len() as f64
    };

    // Build per-scenario, per-task pass rates
    let mut scenario_task_pass_rates: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for sr in scenario_results {
        if !sr.task_results.is_empty() {
            let task_rates: HashMap<String, f64> = sr
                .task_results
                .iter()
                .map(|t| (t.task_id.clone(), if t.passed { 1.0 } else { 0.0 }))
                .collect();
            scenario_task_pass_rates.insert(sr.scenario_id.clone(), task_rates);
        }
    }

    EvalMetrics {
        overall_pass_rate,
        dataset_pass_rates,
        scenario_pass_rate,
        workflow_pass_rate,
        total_scenarios,
        passed_scenarios,
        scenario_task_pass_rates,
    }
}

/// Compute pass/fail and pass rate from EvalResults
fn compute_pass_rate(results: &EvalResults) -> (bool, f64) {
    if results.aligned_results.is_empty() {
        return (false, 0.0);
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
    use scouter_types::agent::utils::AssertionTasks;
    use scouter_types::agent::EvalScenario;

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
        use scouter_types::agent::{AssertionTask, ComparisonOperator, EvaluationTaskType};

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

    fn make_default_profiles() -> HashMap<String, AgentEvalProfile> {
        let mut profiles = HashMap::new();
        profiles.insert("agent_a".to_string(), AgentEvalProfile::default());
        profiles
    }

    #[test]
    fn collect_scenario_data_stores_datasets_and_contexts() {
        let mut runner = EvalRunner::new(
            EvalScenarios::new(vec![make_scenario("s1", "Hello")]),
            make_default_profiles(),
        );

        let mut records = HashMap::new();
        let record = EvalRecord::default();
        records.insert("agent_a".to_string(), vec![record]);

        let scenario = runner.scenarios.scenarios[0].clone();

        runner
            .collect_scenario_data(records, "Agent response".to_string(), &scenario)
            .unwrap();

        assert!(runner.scenarios.scenario_datasets.contains_key("s1"));
        let datasets = &runner.scenarios.scenario_datasets["s1"];
        assert!(datasets.contains_key("agent_a"));
        assert_eq!(datasets["agent_a"].records.len(), 1);

        assert!(datasets["agent_a"].records[0]
            .tags
            .contains(&"scouter.eval.scenario_id=s1".to_string()));

        assert!(runner.scenarios.scenario_contexts.contains_key("s1"));
        let ctx = &runner.scenarios.scenario_contexts["s1"];
        assert_eq!(ctx["response"], "Agent response");
    }

    #[test]
    fn collect_scenario_data_duplicate_returns_error() {
        let mut runner = EvalRunner::new(
            EvalScenarios::new(vec![make_scenario("s1", "Hello")]),
            make_default_profiles(),
        );
        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);
        let scenario = runner.scenarios.scenarios[0].clone();

        runner
            .collect_scenario_data(records.clone(), "Response".to_string(), &scenario)
            .unwrap();
        let result = runner.collect_scenario_data(records, "Response again".to_string(), &scenario);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already has data"));
    }

    #[test]
    fn collect_scenario_data_missing_profile_errors() {
        let mut runner = EvalRunner::new(
            EvalScenarios::new(vec![make_scenario("s1", "Hello")]),
            HashMap::new(),
        );

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);

        let scenario = runner.scenarios.scenarios[0].clone();

        let result = runner.collect_scenario_data(records, "Response".to_string(), &scenario);

        assert!(result.is_err());
    }

    #[test]
    fn collect_scenario_data_multiple_aliases() {
        let mut profiles = make_default_profiles();
        profiles.insert("agent_b".to_string(), AgentEvalProfile::default());

        let mut runner = EvalRunner::new(
            EvalScenarios::new(vec![make_scenario("s1", "Hello")]),
            profiles,
        );

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);
        records.insert(
            "agent_b".to_string(),
            vec![EvalRecord::default(), EvalRecord::default()],
        );

        let scenario = runner.scenarios.scenarios[0].clone();
        runner
            .collect_scenario_data(records, "Response".to_string(), &scenario)
            .unwrap();

        let datasets = &runner.scenarios.scenario_datasets["s1"];
        assert_eq!(datasets["agent_a"].records.len(), 1);
        assert_eq!(datasets["agent_b"].records.len(), 2);
    }

    #[test]
    fn evaluate_no_tasks_only_datasets() {
        let mut runner = EvalRunner::new(
            EvalScenarios::new(vec![make_scenario("s1", "Hello")]),
            make_default_profiles(),
        );

        let mut records = HashMap::new();
        records.insert("agent_a".to_string(), vec![EvalRecord::default()]);

        let scenario = runner.scenarios.scenarios[0].clone();
        runner
            .collect_scenario_data(records, "Response".to_string(), &scenario)
            .unwrap();

        let result = runner.evaluate(None).unwrap();

        assert!(result.dataset_results.contains_key("agent_a"));
        assert!(result.scenario_results.is_empty());
        assert!(result.metrics.dataset_pass_rates.contains_key("agent_a"));
        assert_eq!(result.metrics.total_scenarios, 0);
    }

    #[test]
    fn evaluate_with_assertion_tasks() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner =
            EvalRunner::new(EvalScenarios::new(vec![scenario.clone()]), HashMap::new());

        let context = json!({
            "response": "hello world",
            "expected_outcome": "Response contains hello",
            "metadata": null,
        });
        runner
            .scenarios
            .scenario_contexts
            .insert("s1".to_string(), context);

        let result = runner.evaluate(None).unwrap();

        assert_eq!(result.scenario_results.len(), 1);
        assert_eq!(result.scenario_results[0].scenario_id, "s1");
        assert!(result.scenario_results[0].passed);
        assert_eq!(result.scenario_results[0].pass_rate, 1.0);
        assert_eq!(result.metrics.total_scenarios, 1);
        assert_eq!(result.metrics.passed_scenarios, 1);
        assert_eq!(result.metrics.scenario_pass_rate, 1.0);
    }

    #[test]
    fn evaluate_with_failing_assertion() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner =
            EvalRunner::new(EvalScenarios::new(vec![scenario.clone()]), HashMap::new());

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
    fn evaluate_with_assertion_tasks_populates_task_results() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner =
            EvalRunner::new(EvalScenarios::new(vec![scenario.clone()]), HashMap::new());

        let context = json!({
            "response": "hello world",
            "expected_outcome": "Response contains hello",
            "metadata": null,
        });
        runner
            .scenarios
            .scenario_contexts
            .insert("s1".to_string(), context);

        let result = runner.evaluate(None).unwrap();

        let sr = &result.scenario_results[0];
        assert_eq!(sr.task_results.len(), 1);
        assert_eq!(sr.task_results[0].task_id, "check_response");
        assert!(sr.task_results[0].passed);
        assert_eq!(sr.task_results[0].value, 1.0);
    }

    #[test]
    fn evaluate_with_failing_assertion_task_results() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner =
            EvalRunner::new(EvalScenarios::new(vec![scenario.clone()]), HashMap::new());

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

        let sr = &result.scenario_results[0];
        assert_eq!(sr.task_results.len(), 1);
        assert_eq!(sr.task_results[0].task_id, "check_response");
        assert!(!sr.task_results[0].passed);
        assert_eq!(sr.task_results[0].value, 0.0);

        let rates = &result.metrics.scenario_task_pass_rates;
        assert!(rates.contains_key("s1"));
        assert_eq!(rates["s1"]["check_response"], 0.0);
    }

    #[test]
    fn compute_metrics_scenario_task_pass_rates() {
        use crate::evaluate::scenario_results::{ScenarioResult, TaskSummary};
        use crate::evaluate::types::EvalResults;

        let make_result = |id: &str, passed: bool| ScenarioResult {
            scenario_id: id.to_string(),
            initial_query: "q".to_string(),
            eval_results: EvalResults::new(),
            passed,
            pass_rate: if passed { 1.0 } else { 0.0 },
            task_results: vec![
                TaskSummary {
                    task_id: "t1".to_string(),
                    passed,
                    value: if passed { 1.0 } else { 0.0 },
                },
                TaskSummary {
                    task_id: "t2".to_string(),
                    passed: true,
                    value: 1.0,
                },
            ],
        };

        let scenario_results = vec![make_result("s1", true), make_result("s2", false)];
        let metrics = compute_metrics(&HashMap::new(), &scenario_results);

        assert!(metrics.scenario_task_pass_rates.contains_key("s1"));
        assert!(metrics.scenario_task_pass_rates.contains_key("s2"));
        assert_eq!(metrics.scenario_task_pass_rates["s1"]["t1"], 1.0);
        assert_eq!(metrics.scenario_task_pass_rates["s2"]["t1"], 0.0);
        assert_eq!(metrics.scenario_task_pass_rates["s1"]["t2"], 1.0);
        assert_eq!(metrics.scenario_task_pass_rates["s2"]["t2"], 1.0);
    }

    #[test]
    fn evaluate_scenario_with_tasks_but_no_context_errors() {
        let scenario = make_scenario_with_tasks("s1", "Say hello");
        let mut runner = EvalRunner::new(EvalScenarios::new(vec![scenario]), HashMap::new());

        let result = runner.evaluate(None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("no context"));
    }

    #[test]
    fn compute_pass_rate_empty_results() {
        let results = EvalResults::new();
        let (passed, rate) = compute_pass_rate(&results);
        assert!(!passed);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn compute_pass_rate_zero_tasks() {
        let mut results = EvalResults::new();
        let record = EvalRecord::default();
        let eval_set = scouter_types::agent::EvalSet::new(vec![], Default::default());
        results.add_success(&record, eval_set, BTreeMap::new());

        let (passed, rate) = compute_pass_rate(&results);
        assert!(!passed);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn evaluate_multiple_scenarios_mixed_results() {
        let s_pass = make_scenario_with_tasks("s_pass", "Say hello");
        let s_fail = make_scenario_with_tasks("s_fail", "Say hello");
        let mut runner = EvalRunner::new(EvalScenarios::new(vec![s_pass, s_fail]), HashMap::new());

        runner.scenarios.scenario_contexts.insert(
            "s_pass".to_string(),
            json!({"response": "hello world", "expected_outcome": null, "metadata": null}),
        );
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
                task_results: vec![],
            },
            ScenarioResult {
                scenario_id: "s2".to_string(),
                initial_query: "Q2".to_string(),
                eval_results: EvalResults::new(),
                passed: false,
                pass_rate: 0.5,
                task_results: vec![],
            },
        ];

        let metrics = compute_metrics(&HashMap::new(), &scenario_results);

        assert_eq!(metrics.total_scenarios, 2);
        assert_eq!(metrics.passed_scenarios, 1);
        assert_eq!(metrics.scenario_pass_rate, 0.5);
        assert_eq!(metrics.overall_pass_rate, 0.5);
    }
}
