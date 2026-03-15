use crate::error::EvaluationError;
use crate::evaluate::compare::compare_results;
use crate::evaluate::types::{ComparisonResults, EvalResults};
use owo_colors::OwoColorize;
use potato_head::PyHelperFuncs;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tabled::Tabled;
use tabled::{
    settings::{object::Rows, Alignment, Color, Format, Style},
    Table,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[pyclass]
pub struct EvalMetrics {
    #[pyo3(get)]
    pub overall_pass_rate: f64,

    // HashMap<K,V> fields cannot use #[pyo3(get)]; exposed via the getter method below
    pub dataset_pass_rates: HashMap<String, f64>,

    #[pyo3(get)]
    pub scenario_pass_rate: f64,

    #[pyo3(get)]
    pub total_scenarios: usize,

    #[pyo3(get)]
    pub passed_scenarios: usize,
}

#[derive(Tabled)]
struct MetricEntry {
    #[tabled(rename = "Metric")]
    metric: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Tabled)]
struct DatasetPassRateEntry {
    #[tabled(rename = "Alias")]
    alias: String,
    #[tabled(rename = "Pass Rate")]
    pass_rate: String,
}

#[pymethods]
impl EvalMetrics {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn dataset_pass_rates(&self) -> HashMap<String, f64> {
        self.dataset_pass_rates.clone()
    }

    pub fn as_table(&self) {
        println!("\n{}", "Aggregate Metrics".truecolor(245, 77, 85).bold());

        let entries = vec![
            MetricEntry {
                metric: "Overall Pass Rate".to_string(),
                value: format!("{:.1}%", self.overall_pass_rate * 100.0),
            },
            MetricEntry {
                metric: "Scenario Pass Rate".to_string(),
                value: format!("{:.1}%", self.scenario_pass_rate * 100.0),
            },
            MetricEntry {
                metric: "Total Scenarios".to_string(),
                value: self.total_scenarios.to_string(),
            },
            MetricEntry {
                metric: "Passed Scenarios".to_string(),
                value: self.passed_scenarios.to_string(),
            },
        ];

        let mut table = Table::new(entries);
        table.with(Style::sharp());
        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );
        println!("{}", table);

        if !self.dataset_pass_rates.is_empty() {
            println!("\n{}", "Sub-Agent Pass Rates".truecolor(245, 77, 85).bold());

            let mut alias_entries: Vec<_> = self
                .dataset_pass_rates
                .iter()
                .map(|(alias, rate)| DatasetPassRateEntry {
                    alias: alias.clone(),
                    pass_rate: format!("{:.1}%", rate * 100.0),
                })
                .collect();
            alias_entries.sort_by(|a, b| a.alias.cmp(&b.alias));

            let mut alias_table = Table::new(alias_entries);
            alias_table.with(Style::sharp());
            alias_table.modify(
                Rows::new(0..1),
                (
                    Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                    Alignment::center(),
                    Color::BOLD,
                ),
            );
            println!("{}", alias_table);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ScenarioResult {
    #[pyo3(get)]
    pub scenario_id: String,

    #[pyo3(get)]
    pub initial_query: String,

    pub eval_results: EvalResults,

    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub pass_rate: f64,
}

impl PartialEq for ScenarioResult {
    fn eq(&self, other: &Self) -> bool {
        self.scenario_id == other.scenario_id
            && self.initial_query == other.initial_query
            && self.passed == other.passed
            && self.pass_rate == other.pass_rate
    }
}

#[pymethods]
impl ScenarioResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn eval_results(&self) -> EvalResults {
        self.eval_results.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[pyclass]
pub struct ScenarioDelta {
    #[pyo3(get)]
    pub scenario_id: String,

    #[pyo3(get)]
    pub initial_query: String,

    #[pyo3(get)]
    pub baseline_passed: bool,

    #[pyo3(get)]
    pub comparison_passed: bool,

    #[pyo3(get)]
    pub baseline_pass_rate: f64,

    #[pyo3(get)]
    pub comparison_pass_rate: f64,

    #[pyo3(get)]
    pub status_changed: bool,
}

#[pymethods]
impl ScenarioDelta {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ScenarioComparisonResults {
    pub dataset_comparisons: HashMap<String, ComparisonResults>,

    #[pyo3(get)]
    pub scenario_deltas: Vec<ScenarioDelta>,

    #[pyo3(get)]
    pub baseline_overall_pass_rate: f64,

    #[pyo3(get)]
    pub comparison_overall_pass_rate: f64,

    #[pyo3(get)]
    pub regressed: bool,

    #[pyo3(get)]
    pub improved_aliases: Vec<String>,

    #[pyo3(get)]
    pub regressed_aliases: Vec<String>,
}

impl PartialEq for ScenarioComparisonResults {
    fn eq(&self, other: &Self) -> bool {
        self.scenario_deltas == other.scenario_deltas
            && self.baseline_overall_pass_rate == other.baseline_overall_pass_rate
            && self.comparison_overall_pass_rate == other.comparison_overall_pass_rate
            && self.regressed == other.regressed
            && self.improved_aliases == other.improved_aliases
            && self.regressed_aliases == other.regressed_aliases
        // dataset_comparisons excluded: ComparisonResults does not implement PartialEq
    }
}

#[derive(Tabled)]
struct DatasetComparisonEntry {
    #[tabled(rename = "Alias")]
    alias: String,
    #[tabled(rename = "Delta")]
    delta: String,
    #[tabled(rename = "Status")]
    status: String,
}

#[derive(Tabled)]
struct ScenarioDeltaEntry {
    #[tabled(rename = "Scenario ID")]
    scenario_id: String,
    #[tabled(rename = "Baseline")]
    baseline: String,
    #[tabled(rename = "Current")]
    current: String,
    #[tabled(rename = "Change")]
    change: String,
}

#[pymethods]
impl ScenarioComparisonResults {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn dataset_comparisons(&self) -> HashMap<String, ComparisonResults> {
        self.dataset_comparisons.clone()
    }

    pub fn as_table(&self) {
        println!("\n{}", "Sub-Agent Comparison".truecolor(245, 77, 85).bold());

        if !self.dataset_comparisons.is_empty() {
            let mut alias_entries: Vec<_> = self
                .dataset_comparisons
                .iter()
                .map(|(alias, comp)| {
                    let delta = comp.mean_pass_rate_delta * 100.0;
                    let delta_str = format!("{:+.1}%", delta);
                    let colored_delta = if delta > 1.0 {
                        delta_str.green().to_string()
                    } else if delta < -1.0 {
                        delta_str.red().to_string()
                    } else {
                        delta_str.yellow().to_string()
                    };
                    let status = if comp.regressed {
                        "⚠ REGRESSION".red().to_string()
                    } else if comp.improved_workflows > 0 {
                        "✓ IMPROVED".green().to_string()
                    } else {
                        "→ UNCHANGED".yellow().to_string()
                    };
                    DatasetComparisonEntry {
                        alias: alias.clone(),
                        delta: colored_delta,
                        status,
                    }
                })
                .collect();
            alias_entries.sort_by(|a, b| a.alias.cmp(&b.alias));

            let mut alias_table = Table::new(alias_entries);
            alias_table.with(Style::sharp());
            alias_table.modify(
                Rows::new(0..1),
                (
                    Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                    Alignment::center(),
                    Color::BOLD,
                ),
            );
            println!("{}", alias_table);
        }

        let changed: Vec<_> = self
            .scenario_deltas
            .iter()
            .filter(|d| d.status_changed)
            .collect();

        if !changed.is_empty() {
            println!("\n{}", "Scenario Changes".truecolor(245, 77, 85).bold());

            let entries: Vec<_> = changed
                .iter()
                .map(|d| {
                    let baseline_str = if d.baseline_passed {
                        "✓ PASS".green().to_string()
                    } else {
                        "✗ FAIL".red().to_string()
                    };
                    let current_str = if d.comparison_passed {
                        "✓ PASS".green().to_string()
                    } else {
                        "✗ FAIL".red().to_string()
                    };
                    let change = match (d.baseline_passed, d.comparison_passed) {
                        (true, false) => "Pass → Fail".red().bold().to_string(),
                        (false, true) => "Fail → Pass".green().bold().to_string(),
                        _ => "No Change".yellow().to_string(),
                    };
                    ScenarioDeltaEntry {
                        scenario_id: d.scenario_id.chars().take(16).collect::<String>(),
                        baseline: baseline_str,
                        current: current_str,
                        change,
                    }
                })
                .collect();

            let mut delta_table = Table::new(entries);
            delta_table.with(Style::sharp());
            delta_table.modify(
                Rows::new(0..1),
                (
                    Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                    Alignment::center(),
                    Color::BOLD,
                ),
            );
            println!("{}", delta_table);
        }

        let overall_status = if self.regressed {
            "⚠️  REGRESSION DETECTED".red().bold().to_string()
        } else if !self.improved_aliases.is_empty() {
            "✅ IMPROVEMENT DETECTED".green().bold().to_string()
        } else {
            "➡️  NO SIGNIFICANT CHANGE".yellow().bold().to_string()
        };

        println!("\n{}", "Summary".truecolor(245, 77, 85).bold());
        println!("  Overall Status: {}", overall_status);
        println!(
            "  Baseline Pass Rate: {:.1}%",
            self.baseline_overall_pass_rate * 100.0
        );
        println!(
            "  Current Pass Rate:  {:.1}%",
            self.comparison_overall_pass_rate * 100.0
        );
        if !self.regressed_aliases.is_empty() {
            println!(
                "  Regressed aliases: {}",
                self.regressed_aliases.join(", ").red()
            );
        }
        if !self.improved_aliases.is_empty() {
            println!(
                "  Improved aliases: {}",
                self.improved_aliases.join(", ").green()
            );
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ScenarioEvalResults {
    pub dataset_results: HashMap<String, EvalResults>,

    pub scenario_results: Vec<ScenarioResult>,

    #[pyo3(get)]
    pub metrics: EvalMetrics,
}

impl PartialEq for ScenarioEvalResults {
    fn eq(&self, other: &Self) -> bool {
        self.scenario_results == other.scenario_results && self.metrics == other.metrics
        // dataset_results excluded: EvalResults does not implement PartialEq
    }
}

#[derive(Tabled)]
struct ScenarioResultEntry {
    #[tabled(rename = "Scenario ID")]
    scenario_id: String,
    #[tabled(rename = "Initial Query")]
    initial_query: String,
    #[tabled(rename = "Pass Rate")]
    pass_rate: String,
    #[tabled(rename = "Status")]
    status: String,
}

#[pymethods]
impl ScenarioEvalResults {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> Result<String, EvaluationError> {
        serde_json::to_string(self).map_err(Into::into)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<Self, EvaluationError> {
        serde_json::from_str(&json_string).map_err(Into::into)
    }

    #[getter]
    pub fn dataset_results(&self) -> HashMap<String, EvalResults> {
        self.dataset_results.clone()
    }

    #[getter]
    pub fn scenario_results(&self) -> Vec<ScenarioResult> {
        self.scenario_results.clone()
    }

    pub fn get_scenario_detail(
        &self,
        scenario_id: &str,
    ) -> Result<ScenarioResult, EvaluationError> {
        self.scenario_results
            .iter()
            .find(|r| r.scenario_id == scenario_id)
            .cloned()
            .ok_or_else(|| EvaluationError::MissingKeyError(scenario_id.to_string()))
    }

    #[pyo3(signature = (baseline, regression_threshold = 0.05))]
    pub fn compare_to(
        &self,
        baseline: &ScenarioEvalResults,
        regression_threshold: f64,
    ) -> Result<ScenarioComparisonResults, EvaluationError> {
        let mut dataset_comparisons = HashMap::new();
        let mut improved_aliases = Vec::new();
        let mut regressed_aliases = Vec::new();

        for (alias, current_results) in &self.dataset_results {
            if let Some(baseline_results) = baseline.dataset_results.get(alias) {
                let comp =
                    compare_results(baseline_results, current_results, regression_threshold)?;
                if comp.regressed {
                    regressed_aliases.push(alias.clone());
                } else if comp.improved_workflows > 0 {
                    improved_aliases.push(alias.clone());
                }
                dataset_comparisons.insert(alias.clone(), comp);
            }
        }

        let baseline_scenario_map: HashMap<_, _> = baseline
            .scenario_results
            .iter()
            .map(|r| (r.scenario_id.as_str(), r))
            .collect();

        let mut scenario_deltas = Vec::new();
        for current in &self.scenario_results {
            if let Some(base) = baseline_scenario_map.get(current.scenario_id.as_str()) {
                scenario_deltas.push(ScenarioDelta {
                    scenario_id: current.scenario_id.clone(),
                    initial_query: current.initial_query.clone(),
                    baseline_passed: base.passed,
                    comparison_passed: current.passed,
                    baseline_pass_rate: base.pass_rate,
                    comparison_pass_rate: current.pass_rate,
                    status_changed: base.passed != current.passed,
                });
            }
        }

        Ok(ScenarioComparisonResults {
            dataset_comparisons,
            scenario_deltas,
            baseline_overall_pass_rate: baseline.metrics.overall_pass_rate,
            comparison_overall_pass_rate: self.metrics.overall_pass_rate,
            regressed: !regressed_aliases.is_empty(),
            improved_aliases,
            regressed_aliases,
        })
    }

    pub fn as_table(&self) {
        self.metrics.as_table();

        if !self.scenario_results.is_empty() {
            println!("\n{}", "Scenario Results".truecolor(245, 77, 85).bold());

            let entries: Vec<_> = self
                .scenario_results
                .iter()
                .map(|r| {
                    let query = if r.initial_query.chars().count() > 40 {
                        format!(
                            "{}...",
                            r.initial_query.chars().take(40).collect::<String>()
                        )
                    } else {
                        r.initial_query.clone()
                    };
                    let status = if r.passed {
                        "✓ PASS".green().to_string()
                    } else {
                        "✗ FAIL".red().to_string()
                    };
                    ScenarioResultEntry {
                        scenario_id: r.scenario_id.chars().take(16).collect::<String>(),
                        initial_query: query,
                        pass_rate: format!("{:.1}%", r.pass_rate * 100.0),
                        status,
                    }
                })
                .collect();

            let mut table = Table::new(entries);
            table.with(Style::sharp());
            table.modify(
                Rows::new(0..1),
                (
                    Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                    Alignment::center(),
                    Color::BOLD,
                ),
            );
            println!("{}", table);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluate::types::EvalResults;

    fn empty_metrics(
        overall: f64,
        scenario_pass_rate: f64,
        total: usize,
        passed: usize,
    ) -> EvalMetrics {
        EvalMetrics {
            overall_pass_rate: overall,
            dataset_pass_rates: HashMap::new(),
            scenario_pass_rate,
            total_scenarios: total,
            passed_scenarios: passed,
        }
    }

    fn make_scenario_result(id: &str, query: &str, passed: bool, pass_rate: f64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: id.to_string(),
            initial_query: query.to_string(),
            eval_results: EvalResults::new(),
            passed,
            pass_rate,
        }
    }

    fn make_eval_results() -> ScenarioEvalResults {
        ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![
                make_scenario_result("s1", "Make pasta", true, 1.0),
                make_scenario_result("s2", "Make curry", false, 0.5),
            ],
            metrics: empty_metrics(0.75, 0.5, 2, 1),
        }
    }

    #[test]
    fn eval_metrics_fields() {
        let m = empty_metrics(0.9, 0.85, 100, 85);
        assert_eq!(m.overall_pass_rate, 0.9);
        assert_eq!(m.scenario_pass_rate, 0.85);
        assert_eq!(m.total_scenarios, 100);
        assert_eq!(m.passed_scenarios, 85);
    }

    #[test]
    fn scenario_result_fields() {
        let r = make_scenario_result("id-1", "hello", true, 1.0);
        assert_eq!(r.scenario_id, "id-1");
        assert!(r.passed);
        assert_eq!(r.pass_rate, 1.0);
    }

    #[test]
    fn model_dump_json_roundtrip() {
        let results = make_eval_results();
        let json = results.model_dump_json().unwrap();
        let loaded = ScenarioEvalResults::model_validate_json(json).unwrap();
        assert_eq!(loaded.scenario_results.len(), 2);
        assert_eq!(loaded.metrics.total_scenarios, 2);
        assert_eq!(loaded.metrics.passed_scenarios, 1);
        assert_eq!(loaded.metrics.overall_pass_rate, 0.75);
        assert_eq!(loaded.scenario_results[0].scenario_id, "s1");
        assert_eq!(loaded.scenario_results[1].scenario_id, "s2");
        assert_eq!(loaded.scenario_results[0].pass_rate, 1.0);
        assert_eq!(loaded.scenario_results[1].pass_rate, 0.5);
    }

    #[test]
    fn compare_to_regression_detection() {
        let baseline = make_eval_results();

        let current = ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![
                make_scenario_result("s1", "Make pasta", false, 0.0),
                make_scenario_result("s2", "Make curry", false, 0.5),
            ],
            metrics: empty_metrics(0.25, 0.0, 2, 0),
        };

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert_eq!(comp.scenario_deltas.len(), 2);

        let s1 = comp
            .scenario_deltas
            .iter()
            .find(|d| d.scenario_id == "s1")
            .unwrap();
        assert!(s1.status_changed);
        assert!(s1.baseline_passed);
        assert!(!s1.comparison_passed);

        let s2 = comp
            .scenario_deltas
            .iter()
            .find(|d| d.scenario_id == "s2")
            .unwrap();
        assert!(!s2.status_changed);
    }

    #[test]
    fn compare_to_no_regression() {
        let baseline = make_eval_results();
        let current = make_eval_results();

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert!(!comp.regressed);
        assert!(comp.scenario_deltas.iter().all(|d| !d.status_changed));
    }

    #[test]
    fn compare_to_with_dataset_results() {
        let mut baseline_dataset = HashMap::new();
        baseline_dataset.insert("agent_a".to_string(), EvalResults::new());

        let mut current_dataset = HashMap::new();
        current_dataset.insert("agent_a".to_string(), EvalResults::new());

        let baseline = ScenarioEvalResults {
            dataset_results: baseline_dataset,
            scenario_results: vec![make_scenario_result("s1", "query one", true, 1.0)],
            metrics: empty_metrics(1.0, 1.0, 1, 1),
        };

        let current = ScenarioEvalResults {
            dataset_results: current_dataset,
            scenario_results: vec![make_scenario_result("s1", "query one", false, 0.0)],
            metrics: empty_metrics(0.0, 0.0, 1, 0),
        };

        let comp = current.compare_to(&baseline, 0.05).unwrap();

        // dataset_comparisons should be populated for agent_a
        assert!(comp.dataset_comparisons.contains_key("agent_a"));

        // scenario delta for s1 should show regression
        let s1 = comp
            .scenario_deltas
            .iter()
            .find(|d| d.scenario_id == "s1")
            .unwrap();
        assert!(s1.status_changed);
        assert!(s1.baseline_passed);
        assert!(!s1.comparison_passed);

        // regressed flag reflects true state
        // no dataset-level regression (empty EvalResults → no workflows to regress)
        assert!(comp.improved_aliases.is_empty());
    }

    #[test]
    fn get_scenario_detail_found() {
        let results = make_eval_results();
        let detail = results.get_scenario_detail("s1").unwrap();
        assert_eq!(detail.scenario_id, "s1");
    }

    #[test]
    fn get_scenario_detail_missing() {
        let results = make_eval_results();
        assert!(results.get_scenario_detail("nonexistent").is_err());
    }
}
