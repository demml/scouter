use crate::error::EvaluationError;
use crate::evaluate::types::{
    ComparisonResults, GenAIEvalResults, MissingTask, TaskComparison, WorkflowComparison,
};
use std::collections::HashMap;

/// Compares two GenAIEvalResults datasets and produces a ComparisonResults summary.
///
/// Every workflow is compared against the baseline. The comparison identifies:
/// - Tasks that passed/failed in both datasets
/// - Tasks that changed status between baseline and comparison
/// - Tasks missing in either dataset
/// - Overall pass rate deltas and regression detection
///
/// # Arguments
///
/// * `baseline` - The baseline evaluation results to compare against
/// * `comparison` - The evaluation results being compared
/// * `regression_threshold` - Pass rate delta threshold to flag as regression
///
/// # Returns
///
/// A `ComparisonResults` struct containing workflow comparisons, task-level changes,
/// and aggregate statistics.
///
/// # Errors
///
/// Returns `EvaluationError` if comparison processing fails.
///
/// # Algorithm
///
/// 1. Map baseline and comparison results by `record_uid`, filtering for successful runs
/// 2. For each record present in both datasets:
///    - Build task maps keyed by `task_id`
///    - Compare task pass/fail status for all matched tasks
///    - Track tasks only in baseline or comparison
/// 3. Aggregate workflow-level statistics (pass rates, deltas, regressions)
pub fn compare_results(
    baseline: &GenAIEvalResults,
    comparison: &GenAIEvalResults,
    regression_threshold: f64,
) -> Result<ComparisonResults, EvaluationError> {
    let baseline_map: HashMap<_, _> = baseline
        .aligned_results
        .iter()
        .filter(|r| r.success)
        .map(|r| (r.record_uid.as_str(), r))
        .collect();

    let comparison_map: HashMap<_, _> = comparison
        .aligned_results
        .iter()
        .filter(|r| r.success)
        .map(|r| (r.record_uid.as_str(), r))
        .collect();

    let mut workflow_comparisons = Vec::new();
    let mut task_status_changes = Vec::new();
    let mut missing_tasks = Vec::new();

    for (record_uid, baseline_result) in &baseline_map {
        if let Some(comparison_result) = comparison_map.get(record_uid) {
            let baseline_task_map: HashMap<_, _> = baseline_result
                .eval_set
                .records
                .iter()
                .map(|t| (t.task_id.as_str(), t))
                .collect();

            let comparison_task_map: HashMap<_, _> = comparison_result
                .eval_set
                .records
                .iter()
                .map(|t| (t.task_id.as_str(), t))
                .collect();

            let mut workflow_task_comparisons = Vec::new();
            let mut matched_baseline_passed = 0;
            let mut matched_comparison_passed = 0;
            let mut total_matched = 0;

            for (task_id, baseline_task) in &baseline_task_map {
                if let Some(comparison_task) = comparison_task_map.get(task_id) {
                    let status_changed = baseline_task.passed != comparison_task.passed;

                    if baseline_task.passed {
                        matched_baseline_passed += 1;
                    }
                    if comparison_task.passed {
                        matched_comparison_passed += 1;
                    }
                    total_matched += 1;

                    let task_comp = TaskComparison {
                        task_id: task_id.to_string(),
                        baseline_passed: baseline_task.passed,
                        comparison_passed: comparison_task.passed,
                        status_changed,
                        record_uid: (*record_uid).to_string(),
                    };

                    workflow_task_comparisons.push(task_comp.clone());

                    if status_changed {
                        task_status_changes.push(task_comp.clone());
                    }
                } else {
                    missing_tasks.push(MissingTask {
                        task_id: task_id.to_string(),
                        present_in: "baseline_only".to_string(),
                    });
                }
            }

            for task_id in comparison_task_map.keys() {
                if !baseline_task_map.contains_key(task_id) {
                    missing_tasks.push(MissingTask {
                        task_id: task_id.to_string(),
                        present_in: "comparison_only".to_string(),
                    });
                }
            }

            let baseline_pass_rate = if total_matched > 0 {
                matched_baseline_passed as f64 / total_matched as f64
            } else {
                0.0
            };

            let comparison_pass_rate = if total_matched > 0 {
                matched_comparison_passed as f64 / total_matched as f64
            } else {
                0.0
            };

            let pass_rate_delta = comparison_pass_rate - baseline_pass_rate;
            let is_regression = pass_rate_delta < -regression_threshold;

            workflow_comparisons.push(WorkflowComparison {
                baseline_uid: (*record_uid).to_string(),
                comparison_uid: (*record_uid).to_string(),
                baseline_pass_rate,
                comparison_pass_rate,
                pass_rate_delta,
                is_regression,
                task_comparisons: workflow_task_comparisons,
            });
        }
    }

    let (improved, regressed, unchanged) =
        workflow_comparisons
            .iter()
            .fold((0, 0, 0), |(i, r, u), wc| {
                if wc.is_regression {
                    (i, r + 1, u)
                } else if wc.pass_rate_delta > 0.01 {
                    (i + 1, r, u)
                } else {
                    (i, r, u + 1)
                }
            });

    let mean_delta = if !workflow_comparisons.is_empty() {
        workflow_comparisons
            .iter()
            .map(|wc| wc.pass_rate_delta)
            .sum::<f64>()
            / workflow_comparisons.len() as f64
    } else {
        0.0
    };

    let has_regressed = regressed > 0;

    Ok(ComparisonResults {
        workflow_comparisons,
        total_workflows: baseline_map.len().min(comparison_map.len()),
        improved_workflows: improved,
        regressed_workflows: regressed,
        unchanged_workflows: unchanged,
        mean_pass_rate_delta: mean_delta,
        task_status_changes,
        missing_tasks: missing_tasks,
        baseline_workflow_count: baseline.aligned_results.len(),
        comparison_workflow_count: comparison.aligned_results.len(),
        regressed: has_regressed,
    })
}
