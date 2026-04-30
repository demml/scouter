use crate::agent::{EvalDataset, evaluate_genai_dataset};
use crate::error::EvaluationError;
use crate::evaluate::evaluator::AgentEvaluator;
use crate::evaluate::scenario_results::{
    EvalMetrics, ScenarioEvalResults, ScenarioResult, TaskSummary,
};
use crate::evaluate::types::{EvalResults, EvaluationConfig};
use crate::scenario::EvalScenarios;
use pyo3::prelude::*;
use scouter_state::app_state;
use scouter_types::TraceId as ScouterTraceId;
use scouter_types::agent::EvalScenario;
use scouter_types::agent::{AgentEvalConfig, AgentEvalProfile};
use scouter_types::depythonize_object_to_value;
use scouter_types::trace::sql::TraceSpan;
use scouter_types::trace::{
    SCOUTER_ENTITY, SCOUTER_EVAL_SCENARIO_ID_ATTR, SCOUTER_QUEUE_RECORD, TraceSpanRecord,
    build_trace_spans,
};
use scouter_types::{EvalRecord, EvalRecordSource};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, error};

#[derive(Default)]
struct TraceDispatchCandidate {
    has_queue_path: bool,
    entity_uids: HashSet<String>,
}

/// Stateful evaluation engine that orchestrates scenario evaluation.
///
/// `EvalRunner` owns the scenario definitions and profiles (as `Arc`s),
/// mirroring the `ScouterQueue` pattern. It provides:
/// - `collect_scenario_data()`: Populates scenario datasets and contexts
/// - `evaluate_scenario()`: Runs evaluation for a single scenario (per-scenario scoped)
/// - `finalize()`: Aggregates scenario results into top-level metrics
#[derive(Debug)]
#[pyclass(skip_from_py_object)]
pub struct EvalRunner {
    profiles: HashMap<String, Arc<AgentEvalProfile>>,
    scenarios: EvalScenarios,
    capture_run_id: Option<String>,
    spans_by_scenario: Option<HashMap<String, Arc<Vec<TraceSpan>>>>,
}

#[pymethods]
impl EvalRunner {
    #[new]
    #[pyo3(signature = (scenarios, profiles, capture_run_id=None))]
    fn py_new(
        scenarios: EvalScenarios,
        profiles: HashMap<String, AgentEvalProfile>,
        capture_run_id: Option<String>,
    ) -> Self {
        Self::with_capture_run_id(scenarios, profiles, capture_run_id)
    }

    #[getter]
    pub fn scenarios(&self) -> EvalScenarios {
        self.scenarios.clone()
    }

    /// Evaluate a single scenario by its ID.
    ///
    /// Drains the capture buffer on first call and caches spans for subsequent calls.
    /// Returns a `ScenarioResult` with per-alias dataset results nested inside.
    pub fn evaluate_scenario(
        &mut self,
        scenario_id: &str,
    ) -> Result<ScenarioResult, EvaluationError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(EvaluationError::AgentEvaluatorError(
                "EvalRunner.evaluate_scenario() cannot be called from within an async context."
                    .to_string(),
            ));
        }

        app_state()
            .handle()
            .block_on(async { self.evaluate_scenario_async(scenario_id, true).await })
    }

    /// Aggregate per-scenario results into top-level `ScenarioEvalResults`.
    #[pyo3(signature = (scenario_results, config=None))]
    pub fn finalize(
        &self,
        scenario_results: Vec<ScenarioResult>,
        config: Option<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        let _ = config;

        let dataset_results = merge_dataset_results(&scenario_results);
        let metrics = compute_metrics(&dataset_results, &scenario_results);

        Ok(ScenarioEvalResults {
            dataset_results,
            scenario_results,
            metrics,
        })
    }

    /// Run multi-level evaluation (backward-compat wrapper).
    #[pyo3(signature = (config=None))]
    pub fn evaluate(
        &mut self,
        config: Option<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        let config = Arc::new(config.unwrap_or_default());

        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(EvaluationError::AgentEvaluatorError(
                "EvalRunner.evaluate() cannot be called from within an async context. \
                 Use evaluate_scenario() + finalize() or call from a synchronous Python context."
                    .to_string(),
            ));
        }

        app_state()
            .handle()
            .block_on(async { self.evaluate_async(&config).await })
    }

    /// Populate scenario data into the internal `EvalScenarios` container.
    ///
    /// Accepts any Python object as `response` — strings, dicts, lists, Pydantic
    /// models, numbers — and converts to `serde_json::Value` via
    /// `depythonize_object_to_value`. Scenario tasks navigate the result via
    /// `context_path = "response.<field>"`.
    ///
    /// # Arguments
    /// * `records` - Map of alias → eval records for this scenario
    /// * `response` - The agent's final response (any JSON-serializable Python object)
    /// * `scenario` - Reference to the scenario definition
    #[pyo3(signature = (records, response, scenario))]
    pub fn collect_scenario_data(
        &mut self,
        py: Python<'_>,
        records: HashMap<String, Vec<EvalRecord>>,
        response: Bound<'_, PyAny>,
        scenario: &EvalScenario,
    ) -> Result<(), EvaluationError> {
        let response_val = depythonize_object_to_value(py, &response)?;
        self.collect_scenario_data_value(records, response_val, scenario)
    }
}

impl EvalRunner {
    pub fn new(scenarios: EvalScenarios, profiles: HashMap<String, AgentEvalProfile>) -> Self {
        Self::with_capture_run_id(scenarios, profiles, None)
    }

    pub fn with_capture_run_id(
        scenarios: EvalScenarios,
        profiles: HashMap<String, AgentEvalProfile>,
        capture_run_id: Option<String>,
    ) -> Self {
        let arc_profiles: HashMap<String, Arc<AgentEvalProfile>> = profiles
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();
        Self {
            profiles: arc_profiles,
            scenarios,
            capture_run_id,
            spans_by_scenario: None,
        }
    }

    fn has_valid_queue_marker(span: &TraceSpanRecord) -> bool {
        let has_marker = |key: &str, value: &Value| {
            key == SCOUTER_QUEUE_RECORD
                && value.as_str().map(|val| !val.is_empty()).unwrap_or(false)
        };

        span.attributes
            .iter()
            .any(|attr| has_marker(&attr.key, &attr.value))
            || span
                .events
                .iter()
                .flat_map(|event| event.attributes.iter())
                .any(|attr| has_marker(&attr.key, &attr.value))
    }

    fn extract_entity_uids(span: &TraceSpanRecord) -> HashSet<String> {
        let mut uids = HashSet::new();
        let collect_uid = |key: &str, value: &Value, out: &mut HashSet<String>| {
            if key.starts_with(SCOUTER_ENTITY)
                && let Some(entity_uid) = value.as_str()
            {
                out.insert(entity_uid.to_string());
            }
        };

        for attr in &span.attributes {
            collect_uid(&attr.key, &attr.value, &mut uids);
        }
        for attr in span.events.iter().flat_map(|event| event.attributes.iter()) {
            collect_uid(&attr.key, &attr.value, &mut uids);
        }

        uids
    }

    fn build_dispatch_candidates(
        scenario_spans: &[TraceSpanRecord],
    ) -> HashMap<ScouterTraceId, TraceDispatchCandidate> {
        let mut candidates: HashMap<ScouterTraceId, TraceDispatchCandidate> = HashMap::new();
        for span in scenario_spans {
            let candidate = candidates.entry(span.trace_id).or_default();
            if Self::has_valid_queue_marker(span) {
                candidate.has_queue_path = true;
            }
            candidate
                .entity_uids
                .extend(Self::extract_entity_uids(span));
        }
        candidates
    }

    fn build_synthetic_trace_dispatch_record(
        scenario_id: &str,
        entity_uid: &str,
        trace_id: ScouterTraceId,
    ) -> EvalRecord {
        let synthetic_key = format!("trace-dispatch:{}:{}", entity_uid, trace_id.to_hex());

        EvalRecord {
            context: Value::Object(Default::default()),
            record_id: synthetic_key.clone(),
            session_id: synthetic_key,
            trace_id: Some(trace_id),
            record_source: EvalRecordSource::TraceDispatch,
            tags: vec![format!("{}={}", SCOUTER_EVAL_SCENARIO_ID_ATTR, scenario_id)],
            entity_uid: entity_uid.to_string(),
            ..Default::default()
        }
    }

    fn synthesize_trace_dispatch_records(
        &mut self,
        raw_by_scenario: &HashMap<String, Vec<TraceSpanRecord>>,
    ) {
        let mut aliases_by_entity_uid: HashMap<String, Vec<(String, Arc<AgentEvalProfile>)>> =
            HashMap::new();
        for (alias, profile) in &self.profiles {
            if profile.has_trace_assertions() {
                aliases_by_entity_uid
                    .entry(profile.config.uid.clone())
                    .or_default()
                    .push((alias.clone(), Arc::clone(profile)));
            }
        }

        for (scenario_id, scenario_spans) in raw_by_scenario {
            let Some(datasets) = self.scenarios.scenario_datasets.get_mut(scenario_id) else {
                continue;
            };
            let candidates = Self::build_dispatch_candidates(scenario_spans);

            for (trace_id, candidate) in candidates {
                if candidate.has_queue_path || candidate.entity_uids.is_empty() {
                    continue;
                }
                for entity_uid in &candidate.entity_uids {
                    let Some(alias_profiles) = aliases_by_entity_uid.get(entity_uid) else {
                        continue;
                    };

                    for (alias, profile) in alias_profiles {
                        let dataset =
                            datasets
                                .entry(alias.clone())
                                .or_insert_with(|| EvalDataset {
                                    records: Arc::new(vec![]),
                                    profile: Arc::clone(profile),
                                    spans: Arc::new(vec![]),
                                });
                        if dataset
                            .records
                            .iter()
                            .any(|record| record.trace_id == Some(trace_id))
                        {
                            continue;
                        }
                        Arc::make_mut(&mut dataset.records).push(
                            Self::build_synthetic_trace_dispatch_record(
                                scenario_id,
                                entity_uid,
                                trace_id,
                            ),
                        );
                    }
                }
            }
        }
    }

    fn scenario_ids(&self) -> HashSet<String> {
        self.scenarios
            .scenarios
            .iter()
            .map(|s| s.id.clone())
            .collect()
    }

    fn drain_spans_once(&mut self) {
        if self.spans_by_scenario.is_some() {
            return;
        }
        let scenario_ids = self.scenario_ids();
        let raw_by_scenario = self
            .capture_run_id
            .as_deref()
            .map(|capture_run_id| {
                let spans = scouter_types::span_capture::drain_and_group_spans_for_scenarios(
                    capture_run_id,
                    &scenario_ids,
                );
                scouter_types::span_capture::disable_capture(capture_run_id);
                spans
            })
            .unwrap_or_default();
        self.synthesize_trace_dispatch_records(&raw_by_scenario);
        self.spans_by_scenario = Some(build_spans_by_scenario(raw_by_scenario));
    }

    /// Peek at spans for a single scenario without draining or disabling capture.
    ///
    /// Used by the per-scenario Python path where `evaluate_scenario()` is called
    /// immediately after each scenario runs and `flush_tracer()` is called.
    /// Spans remain in the buffer so that `on_scenario_complete` hooks can still
    /// access them via `get_local_spans_by_trace_ids`. The buffer is fully drained
    /// only when `_teardown_capture` runs at the end of `EvalOrchestrator.run()`.
    fn peek_scenario_spans(&mut self, scenario_id: &str) -> Arc<Vec<TraceSpan>> {
        let raw_spans = self
            .capture_run_id
            .as_deref()
            .map(|capture_run_id| {
                scouter_types::span_capture::peek_spans_for_scenario(capture_run_id, scenario_id)
            })
            .unwrap_or_default();
        let raw_by_scenario: HashMap<String, Vec<TraceSpanRecord>> = if raw_spans.is_empty() {
            HashMap::new()
        } else {
            HashMap::from([(scenario_id.to_string(), raw_spans)])
        };
        self.synthesize_trace_dispatch_records(&raw_by_scenario);
        raw_by_scenario
            .into_values()
            .next()
            .map(|spans| Arc::new(build_trace_spans(spans)))
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }

    async fn evaluate_scenario_async(
        &mut self,
        scenario_id: &str,
        fresh_drain: bool,
    ) -> Result<ScenarioResult, EvaluationError> {
        let spans_arc = if fresh_drain {
            self.peek_scenario_spans(scenario_id)
        } else {
            self.drain_spans_once();
            let spans_by_scenario = self.spans_by_scenario.as_ref().expect("drained");
            spans_by_scenario
                .get(scenario_id)
                .cloned()
                .unwrap_or_else(|| Arc::new(Vec::new()))
        };

        // Attach spans to datasets for this scenario
        if let Some(datasets) = self.scenarios.scenario_datasets.get_mut(scenario_id) {
            for dataset in datasets.values_mut() {
                dataset.spans = Arc::clone(&spans_arc);
            }
        }

        let scenario = self
            .scenarios
            .scenarios
            .iter()
            .find(|s| s.id == scenario_id)
            .cloned()
            .ok_or_else(|| {
                EvaluationError::MissingKeyError(format!("Scenario '{}' not found", scenario_id))
            })?;

        let config = Arc::new(EvaluationConfig::default());

        // Evaluate per-alias datasets for this scenario
        let mut dataset_results: HashMap<String, EvalResults> = HashMap::new();
        if let Some(datasets) = self.scenarios.scenario_datasets.get(scenario_id) {
            for (alias, dataset) in datasets {
                if dataset.records.is_empty() {
                    continue;
                }
                debug!(
                    "Evaluating sub-agent dataset for alias '{}' in scenario '{}'",
                    alias, scenario_id
                );
                match evaluate_genai_dataset(dataset, &config).await {
                    Ok(eval_results) => {
                        dataset_results.insert(alias.clone(), eval_results);
                    }
                    Err(e) => {
                        error!(
                            "Failed to evaluate dataset for alias '{}' in scenario '{}': {:?}",
                            alias, scenario_id, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        // Evaluate scenario-level tasks
        if !scenario.has_tasks() {
            return Ok(ScenarioResult {
                scenario_id: scenario.id.clone(),
                initial_query: scenario.initial_query.clone(),
                eval_results: EvalResults::new(),
                passed: true,
                pass_rate: 1.0,
                task_results: vec![],
                traces: spans_arc.as_ref().clone(),
                dataset_results,
            });
        }

        let context = self
            .scenarios
            .scenario_contexts
            .get(scenario_id)
            .cloned()
            .ok_or_else(|| {
                EvaluationError::MissingKeyError(format!(
                    "Scenario '{}' has tasks but no context — call collect_scenario_data() first",
                    scenario_id
                ))
            })?;

        let record = EvalRecord {
            context,
            record_id: scenario.id.clone(),
            tags: vec![format!("{}={}", SCOUTER_EVAL_SCENARIO_ID_ATTR, scenario.id)],
            ..Default::default()
        };

        let profile = AgentEvalProfile::build_from_parts_async(
            AgentEvalConfig::default(),
            scenario.tasks.clone(),
            None,
        )
        .await?;
        let profile = Arc::new(profile);

        match AgentEvaluator::process_event_record(&record, profile, spans_arc.clone()).await {
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

                Ok(ScenarioResult {
                    scenario_id: scenario.id.clone(),
                    initial_query: scenario.initial_query.clone(),
                    eval_results,
                    passed,
                    pass_rate,
                    task_results,
                    traces: spans_arc.as_ref().clone(),
                    dataset_results,
                })
            }
            Err(e) => {
                error!("Failed to evaluate scenario '{}': {:?}", scenario.id, e);
                let mut eval_results = EvalResults::new();
                eval_results.add_failure(&record, e.to_string());

                Ok(ScenarioResult {
                    scenario_id: scenario.id.clone(),
                    initial_query: scenario.initial_query.clone(),
                    eval_results,
                    passed: false,
                    pass_rate: 0.0,
                    task_results: vec![],
                    traces: spans_arc.as_ref().clone(),
                    dataset_results,
                })
            }
        }
    }

    fn persist_results(
        &mut self,
        dataset_results: HashMap<String, EvalResults>,
        scenario_results: Vec<ScenarioResult>,
        metrics: EvalMetrics,
    ) {
        self.scenarios.dataset_results = dataset_results;
        self.scenarios.scenario_results = scenario_results;
        self.scenarios.metrics = Some(metrics);
    }

    pub(crate) fn collect_scenario_data_value(
        &mut self,
        records: HashMap<String, Vec<EvalRecord>>,
        response: Value,
        scenario: &EvalScenario,
    ) -> Result<(), EvaluationError> {
        let mut alias_datasets: HashMap<String, EvalDataset> = HashMap::new();
        let scenario_id = scenario.id.clone();

        for (alias, mut alias_records) in records {
            // Tag each record with the scenario_id
            let scenario_tag = format!("{}={}", SCOUTER_EVAL_SCENARIO_ID_ATTR, scenario_id);
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
            "query": scenario.initial_query,
            "response": response,
            "expected_outcome": scenario.expected_outcome,
            "metadata": scenario.metadata,
        });
        self.scenarios
            .scenario_contexts
            .insert(scenario_id, context);

        Ok(())
    }

    /// Run multi-level evaluation against captured spans.
    ///
    /// Drains this runner's scoped span capture buffer once, then evaluates each scenario
    /// with per-scenario scoping (no cross-scenario record or span contamination).
    async fn evaluate_async(
        &mut self,
        _config: &Arc<EvaluationConfig>,
    ) -> Result<ScenarioEvalResults, EvaluationError> {
        let scenario_ids: Vec<String> = self
            .scenarios
            .scenarios
            .iter()
            .map(|s| s.id.clone())
            .collect();

        let mut all_results = Vec::with_capacity(scenario_ids.len());
        for scenario_id in &scenario_ids {
            let result = self.evaluate_scenario_async(scenario_id, false).await?;
            all_results.push(result);
        }

        let dataset_results = merge_dataset_results(&all_results);
        // Only include scenarios with tasks in scenario_results (preserves semantics)
        let scenario_results: Vec<ScenarioResult> = all_results
            .into_iter()
            .filter(|r| !r.task_results.is_empty() || !r.eval_results.aligned_results.is_empty())
            .collect();
        let metrics = compute_metrics(&dataset_results, &scenario_results);

        self.persist_results(
            dataset_results.clone(),
            scenario_results.clone(),
            metrics.clone(),
        );

        Ok(ScenarioEvalResults {
            dataset_results,
            scenario_results,
            metrics,
        })
    }
}

/// Merge per-scenario dataset results into a flat alias→EvalResults map (for compare_to compat).
fn merge_dataset_results(scenario_results: &[ScenarioResult]) -> HashMap<String, EvalResults> {
    let mut merged: HashMap<String, EvalResults> = HashMap::new();
    for sr in scenario_results {
        for (alias, eval_results) in &sr.dataset_results {
            let entry = merged.entry(alias.clone()).or_default();
            for aligned in &eval_results.aligned_results {
                let idx = entry.aligned_results.len();
                if !aligned.record_id.is_empty() {
                    entry.results_by_id.insert(aligned.record_id.clone(), idx);
                }
                entry.aligned_results.push(aligned.clone());
            }
        }
    }
    merged
}

/// Build per-scenario span collections from raw grouped span records.
fn build_spans_by_scenario(
    raw_by_scenario: HashMap<String, Vec<TraceSpanRecord>>,
) -> HashMap<String, Arc<Vec<TraceSpan>>> {
    raw_by_scenario
        .into_iter()
        .map(|(id, records)| (id, Arc::new(build_trace_spans(records))))
        .collect()
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

    let mut all_rates = workflow_rates;
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
    use chrono::Utc;
    use scouter_types::agent::EvalScenario;
    use scouter_types::agent::utils::AssertionTasks;
    use scouter_types::agent::{
        AgentEvalConfig, ComparisonOperator, EvaluationTaskType, SpanFilter, TraceAssertion,
        TraceAssertionTask,
    };
    use scouter_types::span_capture::{
        buffer_captured_spans, disable_capture, drain_captured_spans, peek_spans_for_scenario,
    };
    use scouter_types::trace::{
        Attribute, SCOUTER_EVAL_RUN_ID_ATTR, SCOUTER_EVAL_SCENARIO_ID_ATTR, SCOUTER_QUEUE_EVENT,
        SpanEvent, SpanId, TraceId,
    };

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

    fn clear_capture_buffer(run_id: &str) {
        let _ = drain_captured_spans(run_id);
        disable_capture(run_id);
    }

    fn push_captured_spans(run_id: &str, spans: Vec<TraceSpanRecord>) {
        let scoped = spans
            .into_iter()
            .map(|mut span| {
                span.attributes.push(Attribute {
                    key: SCOUTER_EVAL_RUN_ID_ATTR.to_string(),
                    value: Value::String(run_id.to_string()),
                });
                span
            })
            .collect();
        scouter_types::span_capture::enable_capture(run_id);
        buffer_captured_spans(scoped);
    }

    fn runner_with_capture(
        run_id: &str,
        scenarios: EvalScenarios,
        profiles: HashMap<String, AgentEvalProfile>,
    ) -> EvalRunner {
        EvalRunner::with_capture_run_id(scenarios, profiles, Some(run_id.to_string()))
    }

    fn make_trace_profile(alias: &str, uid: &str, span_name: &str) -> AgentEvalProfile {
        let config = AgentEvalConfig {
            uid: uid.to_string(),
            ..Default::default()
        };

        let task = TraceAssertionTask {
            id: format!("trace_exists_{}", alias),
            assertion: TraceAssertion::span_count(SpanFilter::by_name(span_name.to_string())),
            operator: ComparisonOperator::GreaterThanOrEqual,
            expected_value: json!(1),
            description: None,
            depends_on: vec![],
            task_type: EvaluationTaskType::TraceAssertion,
            result: None,
            condition: false,
        };

        AgentEvalProfile::build_from_parts(
            config,
            AssertionTasks {
                assertion: vec![],
                judge: vec![],
                trace: vec![task],
                agent: vec![],
            },
            Some(alias.to_string()),
        )
        .unwrap()
    }

    fn make_span(
        trace_id: TraceId,
        span_id_seed: u8,
        parent_span_id_seed: Option<u8>,
        span_name: &str,
        attributes: Vec<Attribute>,
        events: Vec<SpanEvent>,
    ) -> TraceSpanRecord {
        let now = Utc::now();
        TraceSpanRecord {
            created_at: now,
            trace_id,
            span_id: SpanId::from_bytes([span_id_seed; 8]),
            parent_span_id: parent_span_id_seed.map(|seed| SpanId::from_bytes([seed; 8])),
            span_name: span_name.to_string(),
            start_time: now,
            end_time: now,
            duration_ms: 1,
            attributes,
            events,
            service_name: "test-service".to_string(),
            ..Default::default()
        }
    }

    fn make_scenario_marker_span(trace_id: TraceId, scenario_id: &str) -> TraceSpanRecord {
        make_span(
            trace_id,
            1,
            None,
            "scouter.eval.wrapper",
            vec![Attribute {
                key: SCOUTER_EVAL_SCENARIO_ID_ATTR.to_string(),
                value: Value::String(scenario_id.to_string()),
            }],
            vec![],
        )
    }

    fn make_entity_span(
        trace_id: TraceId,
        entity_uid: &str,
        scenario_id: &str,
        with_queue_marker: bool,
    ) -> TraceSpanRecord {
        let mut events = vec![];
        if with_queue_marker {
            events.push(SpanEvent {
                timestamp: Utc::now(),
                name: SCOUTER_QUEUE_EVENT.to_string(),
                attributes: vec![Attribute {
                    key: SCOUTER_QUEUE_RECORD.to_string(),
                    value: Value::String("00000000-0000-0000-0000-000000000999".to_string()),
                }],
                dropped_attributes_count: 0,
            });
        }

        make_span(
            trace_id,
            2,
            Some(1),
            "trace_only_work",
            vec![
                Attribute {
                    key: format!("{}.{}", SCOUTER_ENTITY, entity_uid),
                    value: Value::String(entity_uid.to_string()),
                },
                Attribute {
                    key: SCOUTER_EVAL_SCENARIO_ID_ATTR.to_string(),
                    value: Value::String(scenario_id.to_string()),
                },
            ],
            events,
        )
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
            .collect_scenario_data_value(records, json!("Agent response"), &scenario)
            .unwrap();

        assert!(runner.scenarios.scenario_datasets.contains_key("s1"));
        let datasets = &runner.scenarios.scenario_datasets["s1"];
        assert!(datasets.contains_key("agent_a"));
        assert_eq!(datasets["agent_a"].records.len(), 1);

        assert!(
            datasets["agent_a"].records[0]
                .tags
                .contains(&"scouter.eval.scenario_id=s1".to_string())
        );

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
            .collect_scenario_data_value(records.clone(), json!("Response"), &scenario)
            .unwrap();
        let result =
            runner.collect_scenario_data_value(records, json!("Response again"), &scenario);
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

        let result = runner.collect_scenario_data_value(records, json!("Response"), &scenario);

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
            .collect_scenario_data_value(records, json!("Response"), &scenario)
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
            .collect_scenario_data_value(records, json!("Response"), &scenario)
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
            traces: vec![],
            dataset_results: HashMap::new(),
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
                traces: vec![],
                dataset_results: HashMap::new(),
            },
            ScenarioResult {
                scenario_id: "s2".to_string(),
                initial_query: "Q2".to_string(),
                eval_results: EvalResults::new(),
                passed: false,
                pass_rate: 0.5,
                task_results: vec![],
                traces: vec![],
                dataset_results: HashMap::new(),
            },
        ];

        let metrics = compute_metrics(&HashMap::new(), &scenario_results);

        assert_eq!(metrics.total_scenarios, 2);
        assert_eq!(metrics.passed_scenarios, 1);
        assert_eq!(metrics.scenario_pass_rate, 0.5);
        assert_eq!(metrics.overall_pass_rate, 0.5);
    }

    #[test]
    fn evaluate_synthesizes_trace_dispatch_records_for_trace_only_spans() {
        let run_id = "test_synthesize";
        clear_capture_buffer(run_id);
        let scenario = make_scenario("s1", "Hello");
        let entity_uid = "00000000-0000-0000-0000-000000000001".to_string();
        let profile = make_trace_profile("agent_a", &entity_uid, "trace_only_work");
        let mut runner = runner_with_capture(
            run_id,
            EvalScenarios::new(vec![scenario.clone()]),
            HashMap::from([("agent_a".to_string(), profile)]),
        );

        runner
            .collect_scenario_data_value(HashMap::new(), json!("trace only response"), &scenario)
            .unwrap();

        let trace_id = TraceId::from_bytes([1; 16]);
        push_captured_spans(
            run_id,
            vec![
                make_scenario_marker_span(trace_id, &scenario.id),
                make_entity_span(trace_id, &entity_uid, &scenario.id, false),
            ],
        );

        let result = runner.evaluate(None).unwrap();
        let scenario_datasets = runner.scenarios.scenario_datasets.get("s1").unwrap();
        let records = &scenario_datasets["agent_a"].records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_source.as_str(), "trace_dispatch");
        assert_eq!(records[0].trace_id, Some(trace_id));
        assert_eq!(records[0].entity_uid, entity_uid);
        assert!(records[0].record_id.starts_with("trace-dispatch:"));
        assert_eq!(records[0].record_id, records[0].session_id);
        assert!(result.dataset_results.contains_key("agent_a"));
        clear_capture_buffer(run_id);
    }

    #[test]
    fn evaluate_does_not_synthesize_trace_dispatch_when_queue_marker_exists() {
        let run_id = "test_no_queue_marker";
        clear_capture_buffer(run_id);
        let scenario = make_scenario("s1", "Hello");
        let entity_uid = "00000000-0000-0000-0000-000000000002".to_string();
        let profile = make_trace_profile("agent_a", &entity_uid, "trace_only_work");
        let mut runner = runner_with_capture(
            run_id,
            EvalScenarios::new(vec![scenario.clone()]),
            HashMap::from([("agent_a".to_string(), profile)]),
        );

        runner
            .collect_scenario_data_value(HashMap::new(), json!("trace only response"), &scenario)
            .unwrap();

        let trace_id = TraceId::from_bytes([2; 16]);
        push_captured_spans(
            run_id,
            vec![
                make_scenario_marker_span(trace_id, &scenario.id),
                make_entity_span(trace_id, &entity_uid, &scenario.id, true),
            ],
        );

        let result = runner.evaluate(None).unwrap();
        assert!(!result.dataset_results.contains_key("agent_a"));
        let scenario_datasets = runner.scenarios.scenario_datasets.get("s1").unwrap();
        assert!(!scenario_datasets.contains_key("agent_a"));
        clear_capture_buffer(run_id);
    }

    #[test]
    fn evaluate_dedupes_synthesized_trace_dispatch_records() {
        let run_id = "test_dedupes";
        clear_capture_buffer(run_id);
        let scenario = make_scenario("s1", "Hello");
        let entity_uid = "00000000-0000-0000-0000-000000000003".to_string();
        let profile = make_trace_profile("agent_a", &entity_uid, "trace_only_work");
        let mut runner = runner_with_capture(
            run_id,
            EvalScenarios::new(vec![scenario.clone()]),
            HashMap::from([("agent_a".to_string(), profile)]),
        );

        runner
            .collect_scenario_data_value(HashMap::new(), json!("trace only response"), &scenario)
            .unwrap();

        let trace_id = TraceId::from_bytes([3; 16]);
        push_captured_spans(
            run_id,
            vec![
                make_scenario_marker_span(trace_id, &scenario.id),
                make_entity_span(trace_id, &entity_uid, &scenario.id, false),
                make_entity_span(trace_id, &entity_uid, &scenario.id, false),
            ],
        );

        runner.evaluate(None).unwrap();
        let scenario_datasets = runner.scenarios.scenario_datasets.get("s1").unwrap();
        let records = &scenario_datasets["agent_a"].records;
        assert_eq!(records.len(), 1);
        clear_capture_buffer(run_id);
    }

    #[test]
    fn evaluate_skips_synthesis_when_trace_record_already_exists() {
        let run_id = "test_skip_synthesis";
        clear_capture_buffer(run_id);
        let scenario = make_scenario("s1", "Hello");
        let entity_uid = "00000000-0000-0000-0000-000000000004".to_string();
        let profile = make_trace_profile("agent_a", &entity_uid, "trace_only_work");
        let mut runner = runner_with_capture(
            run_id,
            EvalScenarios::new(vec![scenario.clone()]),
            HashMap::from([("agent_a".to_string(), profile)]),
        );

        let trace_id = TraceId::from_bytes([4; 16]);
        let existing = EvalRecord {
            trace_id: Some(trace_id),
            record_id: "user-record".to_string(),
            session_id: "user-session".to_string(),
            ..Default::default()
        };
        runner
            .collect_scenario_data_value(
                HashMap::from([("agent_a".to_string(), vec![existing])]),
                json!("trace only response"),
                &scenario,
            )
            .unwrap();

        push_captured_spans(
            run_id,
            vec![
                make_scenario_marker_span(trace_id, &scenario.id),
                make_entity_span(trace_id, &entity_uid, &scenario.id, false),
            ],
        );

        runner.evaluate(None).unwrap();
        let scenario_datasets = runner.scenarios.scenario_datasets.get("s1").unwrap();
        let records = &scenario_datasets["agent_a"].records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_source.as_str(), "user");
        clear_capture_buffer(run_id);
    }

    #[test]
    fn merge_dataset_results_concatenates_and_indexes() {
        use crate::evaluate::types::AlignedEvalResult;
        use scouter_types::EvalRecord;

        let make_aligned = |record_id: &str| -> AlignedEvalResult {
            let record = EvalRecord {
                record_id: record_id.to_string(),
                ..Default::default()
            };
            AlignedEvalResult::from_failure(&record, "test".to_string())
        };

        let mut ds1 = HashMap::new();
        let mut er1 = EvalResults::new();
        let a = make_aligned("rec-a");
        er1.results_by_id.insert("rec-a".to_string(), 0);
        er1.aligned_results.push(a);
        ds1.insert("agent".to_string(), er1);

        let mut ds2 = HashMap::new();
        let mut er2 = EvalResults::new();
        let b = make_aligned("rec-b");
        er2.results_by_id.insert("rec-b".to_string(), 0);
        er2.aligned_results.push(b);
        ds2.insert("agent".to_string(), er2);

        let make_sr = |id: &str, ds: HashMap<String, EvalResults>| -> ScenarioResult {
            ScenarioResult {
                scenario_id: id.to_string(),
                initial_query: String::new(),
                eval_results: EvalResults::new(),
                passed: true,
                pass_rate: 1.0,
                task_results: vec![],
                traces: vec![],
                dataset_results: ds,
            }
        };
        let sr1 = make_sr("s1", ds1);
        let sr2 = make_sr("s2", ds2);

        let merged = super::merge_dataset_results(&[sr1, sr2]);
        let agent_results = merged.get("agent").expect("alias present");
        assert_eq!(agent_results.aligned_results.len(), 2);
        assert!(agent_results.results_by_id.contains_key("rec-a"));
        assert!(agent_results.results_by_id.contains_key("rec-b"));
    }

    #[test]
    fn evaluate_scenario_peek_does_not_drain_buffer() {
        let run_id = "test-peek-drain-contract";
        let trace_id = TraceId::from_bytes([0xAA; 16]);

        let scenario = make_scenario("s1", "test query");
        let mut runner = runner_with_capture(
            run_id,
            EvalScenarios::new(vec![scenario.clone()]),
            make_default_profiles(),
        );

        // Seed a span tagged with the scenario_id so peek_spans_for_scenario can find it.
        push_captured_spans(
            run_id,
            vec![make_scenario_marker_span(trace_id, &scenario.id)],
        );

        // evaluate_scenario uses peek (not drain) — buffer must survive the call.
        let _ = runner.evaluate_scenario("s1");

        let remaining = peek_spans_for_scenario(run_id, "s1");
        assert!(
            !remaining.is_empty(),
            "evaluate_scenario drained the capture buffer; on_scenario_complete hooks would get empty spans"
        );

        clear_capture_buffer(run_id);
    }
}
