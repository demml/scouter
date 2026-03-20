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
pub struct TaskSummary {
    #[pyo3(get)]
    pub task_id: String,

    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl TaskSummary {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

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

    #[serde(default)]
    pub scenario_task_pass_rates: HashMap<String, HashMap<String, f64>>,
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

    #[getter]
    pub fn scenario_task_pass_rates(&self) -> HashMap<String, HashMap<String, f64>> {
        self.scenario_task_pass_rates.clone()
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

    #[pyo3(get)]
    #[serde(default)]
    pub task_results: Vec<TaskSummary>,
}

impl PartialEq for ScenarioResult {
    fn eq(&self, other: &Self) -> bool {
        self.scenario_id == other.scenario_id
            && self.initial_query == other.initial_query
            && self.passed == other.passed
            && self.pass_rate == other.pass_rate
            && self.task_results == other.task_results
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

    #[pyo3(get)]
    #[serde(default)]
    pub new_aliases: Vec<String>,

    #[pyo3(get)]
    #[serde(default)]
    pub removed_aliases: Vec<String>,

    #[pyo3(get)]
    #[serde(default)]
    pub new_scenarios: Vec<String>,

    #[pyo3(get)]
    #[serde(default)]
    pub removed_scenarios: Vec<String>,

    #[serde(default)]
    pub baseline_alias_pass_rates: HashMap<String, f64>,

    #[serde(default)]
    pub comparison_alias_pass_rates: HashMap<String, f64>,
}

impl PartialEq for ScenarioComparisonResults {
    fn eq(&self, other: &Self) -> bool {
        self.scenario_deltas == other.scenario_deltas
            && self.baseline_overall_pass_rate == other.baseline_overall_pass_rate
            && self.comparison_overall_pass_rate == other.comparison_overall_pass_rate
            && self.regressed == other.regressed
            && self.improved_aliases == other.improved_aliases
            && self.regressed_aliases == other.regressed_aliases
            && self.new_aliases == other.new_aliases
            && self.removed_aliases == other.removed_aliases
            && self.new_scenarios == other.new_scenarios
            && self.removed_scenarios == other.removed_scenarios
            && self.baseline_alias_pass_rates == other.baseline_alias_pass_rates
            && self.comparison_alias_pass_rates == other.comparison_alias_pass_rates
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
    #[tabled(rename = "Pass Rate Δ")]
    pass_rate_delta: String,
    #[tabled(rename = "Change")]
    change: String,
}

#[derive(Tabled)]
struct AliasPassRateEntry {
    #[tabled(rename = "Alias")]
    alias: String,
    #[tabled(rename = "Baseline")]
    baseline: String,
    #[tabled(rename = "Current")]
    current: String,
    #[tabled(rename = "Delta")]
    delta: String,
    #[tabled(rename = "Status")]
    status: String,
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

    #[getter]
    pub fn baseline_alias_pass_rates(&self) -> HashMap<String, f64> {
        self.baseline_alias_pass_rates.clone()
    }

    #[getter]
    pub fn comparison_alias_pass_rates(&self) -> HashMap<String, f64> {
        self.comparison_alias_pass_rates.clone()
    }

    pub fn model_dump_json(&self) -> Result<String, EvaluationError> {
        serde_json::to_string(self).map_err(Into::into)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<Self, EvaluationError> {
        serde_json::from_str(&json_string).map_err(Into::into)
    }

    pub fn save(&self, path: &str) -> Result<(), EvaluationError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load(path: &str) -> Result<Self, EvaluationError> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(Into::into)
    }

    pub fn as_table(&self) {
        // Sub-Agent Comparison with pass rates
        if !self.baseline_alias_pass_rates.is_empty()
            || !self.comparison_alias_pass_rates.is_empty()
        {
            println!("\n{}", "Sub-Agent Comparison".truecolor(245, 77, 85).bold());

            let mut all_aliases: Vec<String> = self
                .baseline_alias_pass_rates
                .keys()
                .chain(self.comparison_alias_pass_rates.keys())
                .cloned()
                .collect();
            all_aliases.sort();
            all_aliases.dedup();

            let mut alias_entries: Vec<AliasPassRateEntry> = Vec::new();
            for alias in &all_aliases {
                let baseline_rate = self.baseline_alias_pass_rates.get(alias);
                let current_rate = self.comparison_alias_pass_rates.get(alias);

                let (baseline_str, current_str, delta_str, status) =
                    match (baseline_rate, current_rate) {
                        (Some(b), Some(c)) => {
                            let delta = (c - b) * 100.0;
                            let d = format!("{:+.1}%", delta);
                            let colored_delta = if delta > 1.0 {
                                d.green().to_string()
                            } else if delta < -1.0 {
                                d.red().to_string()
                            } else {
                                d.yellow().to_string()
                            };
                            let s = if self.regressed_aliases.contains(alias) {
                                "REGRESSION".red().to_string()
                            } else if self.improved_aliases.contains(alias) {
                                "IMPROVED".green().to_string()
                            } else {
                                "UNCHANGED".yellow().to_string()
                            };
                            (
                                format!("{:.1}%", b * 100.0),
                                format!("{:.1}%", c * 100.0),
                                colored_delta,
                                s,
                            )
                        }
                        (None, Some(c)) => (
                            "-".to_string(),
                            format!("{:.1}%", c * 100.0),
                            "-".to_string(),
                            "NEW".green().bold().to_string(),
                        ),
                        (Some(b), None) => (
                            format!("{:.1}%", b * 100.0),
                            "-".to_string(),
                            "-".to_string(),
                            "REMOVED".red().bold().to_string(),
                        ),
                        (None, None) => continue,
                    };

                alias_entries.push(AliasPassRateEntry {
                    alias: alias.clone(),
                    baseline: baseline_str,
                    current: current_str,
                    delta: delta_str,
                    status,
                });
            }

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
        } else if !self.dataset_comparisons.is_empty() {
            // Fallback for old data without alias pass rates
            println!("\n{}", "Sub-Agent Comparison".truecolor(245, 77, 85).bold());

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
                        "REGRESSION".red().to_string()
                    } else if comp.improved_workflows > 0 {
                        "IMPROVED".green().to_string()
                    } else {
                        "UNCHANGED".yellow().to_string()
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

        // All Scenarios table (not just changed ones)
        if !self.scenario_deltas.is_empty() {
            println!("\n{}", "Scenario Comparison".truecolor(245, 77, 85).bold());

            let entries: Vec<_> = self
                .scenario_deltas
                .iter()
                .map(|d| {
                    let baseline_str = if d.baseline_passed {
                        "PASS".green().to_string()
                    } else {
                        "FAIL".red().to_string()
                    };
                    let current_str = if d.comparison_passed {
                        "PASS".green().to_string()
                    } else {
                        "FAIL".red().to_string()
                    };
                    let pr_delta = (d.comparison_pass_rate - d.baseline_pass_rate) * 100.0;
                    let pr_delta_str = format!("{:+.1}%", pr_delta);
                    let colored_pr_delta = if pr_delta > 1.0 {
                        pr_delta_str.green().to_string()
                    } else if pr_delta < -1.0 {
                        pr_delta_str.red().to_string()
                    } else {
                        pr_delta_str.yellow().to_string()
                    };
                    let change = match (d.baseline_passed, d.comparison_passed) {
                        (true, false) => "Pass -> Fail".red().bold().to_string(),
                        (false, true) => "Fail -> Pass".green().bold().to_string(),
                        _ => "-".to_string(),
                    };
                    ScenarioDeltaEntry {
                        scenario_id: d.scenario_id.chars().take(16).collect::<String>(),
                        baseline: baseline_str,
                        current: current_str,
                        pass_rate_delta: colored_pr_delta,
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

        // New/Removed Scenarios
        if !self.new_scenarios.is_empty() {
            println!(
                "\n{}  {}",
                "New Scenarios:".truecolor(245, 77, 85).bold(),
                self.new_scenarios.join(", ").green()
            );
        }
        if !self.removed_scenarios.is_empty() {
            println!(
                "\n{}  {}",
                "Removed Scenarios:".truecolor(245, 77, 85).bold(),
                self.removed_scenarios.join(", ").red()
            );
        }

        // Summary
        let overall_status = if self.regressed {
            "REGRESSION DETECTED".red().bold().to_string()
        } else if !self.improved_aliases.is_empty() {
            "IMPROVEMENT DETECTED".green().bold().to_string()
        } else {
            "NO SIGNIFICANT CHANGE".yellow().bold().to_string()
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
        if !self.new_aliases.is_empty() {
            println!("  New aliases: {}", self.new_aliases.join(", ").green());
        }
        if !self.removed_aliases.is_empty() {
            println!(
                "  Removed aliases: {}",
                self.removed_aliases.join(", ").red()
            );
        }
        let changed_count = self
            .scenario_deltas
            .iter()
            .filter(|d| d.status_changed)
            .count();
        if changed_count > 0 {
            println!("  Scenarios changed: {}", changed_count);
        }
        if !self.new_scenarios.is_empty() {
            println!("  New scenarios: {}", self.new_scenarios.len());
        }
        if !self.removed_scenarios.is_empty() {
            println!("  Removed scenarios: {}", self.removed_scenarios.len());
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
    #[tabled(rename = "Tasks")]
    tasks: usize,
    #[tabled(rename = "Passed")]
    passed_count: usize,
    #[tabled(rename = "Failed")]
    failed_count: usize,
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

    pub fn save(&self, path: &str) -> Result<(), EvaluationError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    #[staticmethod]
    pub fn load(path: &str) -> Result<Self, EvaluationError> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json).map_err(Into::into)
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

        // Detect new/removed aliases
        let mut new_aliases: Vec<String> = self
            .dataset_results
            .keys()
            .filter(|alias| !baseline.dataset_results.contains_key(*alias))
            .cloned()
            .collect();
        new_aliases.sort();

        let mut removed_aliases: Vec<String> = baseline
            .dataset_results
            .keys()
            .filter(|alias| !self.dataset_results.contains_key(*alias))
            .cloned()
            .collect();
        removed_aliases.sort();

        let baseline_scenario_map: HashMap<_, _> = baseline
            .scenario_results
            .iter()
            .map(|r| (r.scenario_id.as_str(), r))
            .collect();

        let current_scenario_map: HashMap<_, _> = self
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

        // Detect new/removed scenarios
        let mut new_scenarios: Vec<String> = current_scenario_map
            .keys()
            .filter(|id| !baseline_scenario_map.contains_key(*id))
            .map(|id| id.to_string())
            .collect();
        new_scenarios.sort();

        let mut removed_scenarios: Vec<String> = baseline_scenario_map
            .keys()
            .filter(|id| !current_scenario_map.contains_key(*id))
            .map(|id| id.to_string())
            .collect();
        removed_scenarios.sort();

        // Per-alias pass rates from metrics
        let baseline_alias_pass_rates = baseline.metrics.dataset_pass_rates.clone();
        let comparison_alias_pass_rates = self.metrics.dataset_pass_rates.clone();

        Ok(ScenarioComparisonResults {
            dataset_comparisons,
            scenario_deltas,
            baseline_overall_pass_rate: baseline.metrics.overall_pass_rate,
            comparison_overall_pass_rate: self.metrics.overall_pass_rate,
            regressed: !regressed_aliases.is_empty(),
            improved_aliases,
            regressed_aliases,
            new_aliases,
            removed_aliases,
            new_scenarios,
            removed_scenarios,
            baseline_alias_pass_rates,
            comparison_alias_pass_rates,
        })
    }

    #[pyo3(signature = (show_datasets=false))]
    pub fn as_table(&mut self, show_datasets: bool) {
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
                    let total = r.task_results.len();
                    let passed_count = r.task_results.iter().filter(|t| t.passed).count();
                    let failed_count = total - passed_count;
                    ScenarioResultEntry {
                        scenario_id: r.scenario_id.chars().take(16).collect::<String>(),
                        initial_query: query,
                        tasks: total,
                        passed_count,
                        failed_count,
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

        if show_datasets {
            let mut aliases: Vec<_> = self.dataset_results.keys().cloned().collect();
            aliases.sort();
            for alias in aliases {
                if let Some(eval_results) = self.dataset_results.get_mut(&alias) {
                    println!(
                        "\n{}",
                        format!("Dataset: {}", alias).truecolor(245, 77, 85).bold()
                    );
                    eval_results.as_table(false);
                }
            }
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
            scenario_task_pass_rates: HashMap::new(),
        }
    }

    fn make_scenario_result(id: &str, query: &str, passed: bool, pass_rate: f64) -> ScenarioResult {
        ScenarioResult {
            scenario_id: id.to_string(),
            initial_query: query.to_string(),
            eval_results: EvalResults::new(),
            passed,
            pass_rate,
            task_results: vec![],
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

    #[test]
    fn save_load_roundtrip() {
        let results = make_eval_results();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.json");
        let path_str = path.to_str().unwrap();

        results.save(path_str).unwrap();
        let loaded = ScenarioEvalResults::load(path_str).unwrap();
        assert_eq!(results, loaded);
    }

    #[test]
    fn comparison_model_dump_json_roundtrip() {
        let comp = ScenarioComparisonResults {
            dataset_comparisons: HashMap::new(),
            scenario_deltas: vec![ScenarioDelta {
                scenario_id: "s1".to_string(),
                initial_query: "hello".to_string(),
                baseline_passed: true,
                comparison_passed: false,
                baseline_pass_rate: 1.0,
                comparison_pass_rate: 0.0,
                status_changed: true,
            }],
            baseline_overall_pass_rate: 1.0,
            comparison_overall_pass_rate: 0.5,
            regressed: true,
            improved_aliases: vec![],
            regressed_aliases: vec!["a".to_string()],
            new_aliases: vec!["c".to_string()],
            removed_aliases: vec!["b".to_string()],
            new_scenarios: vec!["s3".to_string()],
            removed_scenarios: vec![],
            baseline_alias_pass_rates: HashMap::from([("a".to_string(), 1.0)]),
            comparison_alias_pass_rates: HashMap::from([("a".to_string(), 0.5)]),
        };

        let json = comp.model_dump_json().unwrap();
        let loaded = ScenarioComparisonResults::model_validate_json(json).unwrap();
        assert_eq!(comp, loaded);
    }

    #[test]
    fn comparison_save_load_roundtrip() {
        let comp = ScenarioComparisonResults {
            dataset_comparisons: HashMap::new(),
            scenario_deltas: vec![],
            baseline_overall_pass_rate: 0.8,
            comparison_overall_pass_rate: 0.9,
            regressed: false,
            improved_aliases: vec!["x".to_string()],
            regressed_aliases: vec![],
            new_aliases: vec![],
            removed_aliases: vec![],
            new_scenarios: vec![],
            removed_scenarios: vec![],
            baseline_alias_pass_rates: HashMap::new(),
            comparison_alias_pass_rates: HashMap::new(),
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("comp.json");
        let path_str = path.to_str().unwrap();

        comp.save(path_str).unwrap();
        let loaded = ScenarioComparisonResults::load(path_str).unwrap();
        assert_eq!(comp, loaded);
    }

    #[test]
    fn compare_to_new_scenarios() {
        let baseline = make_eval_results(); // s1, s2
        let current = ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![
                make_scenario_result("s1", "Make pasta", true, 1.0),
                make_scenario_result("s2", "Make curry", false, 0.5),
                make_scenario_result("s3", "Make salad", true, 0.8),
            ],
            metrics: empty_metrics(0.77, 0.67, 3, 2),
        };

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert_eq!(comp.new_scenarios, vec!["s3".to_string()]);
        assert!(comp.removed_scenarios.is_empty());
        // Only shared scenarios appear in deltas
        assert_eq!(comp.scenario_deltas.len(), 2);
    }

    #[test]
    fn compare_to_removed_scenarios() {
        let baseline = ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![
                make_scenario_result("s1", "Make pasta", true, 1.0),
                make_scenario_result("s2", "Make curry", false, 0.5),
                make_scenario_result("s3", "Make salad", true, 0.8),
            ],
            metrics: empty_metrics(0.77, 0.67, 3, 2),
        };
        let current = make_eval_results(); // s1, s2

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert_eq!(comp.removed_scenarios, vec!["s3".to_string()]);
        assert!(comp.new_scenarios.is_empty());
    }

    #[test]
    fn compare_to_new_removed_aliases() {
        let mut baseline_datasets = HashMap::new();
        baseline_datasets.insert("a".to_string(), EvalResults::new());
        baseline_datasets.insert("b".to_string(), EvalResults::new());

        let mut current_datasets = HashMap::new();
        current_datasets.insert("a".to_string(), EvalResults::new());
        current_datasets.insert("c".to_string(), EvalResults::new());

        let baseline = ScenarioEvalResults {
            dataset_results: baseline_datasets,
            scenario_results: vec![make_scenario_result("s1", "q", true, 1.0)],
            metrics: empty_metrics(1.0, 1.0, 1, 1),
        };

        let current = ScenarioEvalResults {
            dataset_results: current_datasets,
            scenario_results: vec![make_scenario_result("s1", "q", true, 1.0)],
            metrics: empty_metrics(1.0, 1.0, 1, 1),
        };

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert_eq!(comp.new_aliases, vec!["c".to_string()]);
        assert_eq!(comp.removed_aliases, vec!["b".to_string()]);
    }

    #[test]
    fn compare_to_alias_pass_rates() {
        let mut baseline_metrics = empty_metrics(0.9, 1.0, 1, 1);
        baseline_metrics
            .dataset_pass_rates
            .insert("agent_a".to_string(), 0.9);
        baseline_metrics
            .dataset_pass_rates
            .insert("agent_b".to_string(), 0.8);

        let mut current_metrics = empty_metrics(0.85, 1.0, 1, 1);
        current_metrics
            .dataset_pass_rates
            .insert("agent_a".to_string(), 0.85);
        current_metrics
            .dataset_pass_rates
            .insert("agent_b".to_string(), 0.75);

        let baseline = ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![make_scenario_result("s1", "q", true, 1.0)],
            metrics: baseline_metrics,
        };

        let current = ScenarioEvalResults {
            dataset_results: HashMap::new(),
            scenario_results: vec![make_scenario_result("s1", "q", true, 1.0)],
            metrics: current_metrics,
        };

        let comp = current.compare_to(&baseline, 0.05).unwrap();
        assert_eq!(comp.baseline_alias_pass_rates.get("agent_a"), Some(&0.9));
        assert_eq!(comp.baseline_alias_pass_rates.get("agent_b"), Some(&0.8));
        assert_eq!(comp.comparison_alias_pass_rates.get("agent_a"), Some(&0.85));
        assert_eq!(comp.comparison_alias_pass_rates.get("agent_b"), Some(&0.75));
    }
}
