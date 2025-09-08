
use potato_head::{Embedder, Workflow};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::task::JoinSet;
use tracing::{error, warn};
use crate::types::LLMEvalRecord;
use crate::types::LLMEvalTaskResult;
use crate::types::LLMEvalResults;
use crate::error::EvaluationError;
use crate::types::EvalResult;
use potato_head::StructuredOutput;
use potato_head::{WorkflowError, Score, TaskStatus};




/// Process a workflow result and extract scores from completed tasks
pub fn process_workflow_result(
    workflow_result: Arc<RwLock<Workflow>>,
    data_id: String,
) -> Result<EvalResult, EvaluationError> {
    let mut eval_result = EvalResult {
        error: None,
        tasks: HashMap::new(),
        id: data_id.clone(),
    };

    let workflow = workflow_result
        .read()
        .map_err(|_| WorkflowError::LockAcquireError)?;

    let tasks = workflow.task_list.tasks();

    // iterate of each task and extract score
    for task in tasks.values() {
        if let (TaskStatus::Completed, Some(result)) = (&task.status, &task.result) {
            if let Some(content) = result.content() {
                match Score::model_validate_json_str(&content) {
                    Ok(score) => {
                        eval_result.tasks.insert(task.id.clone(), score);
                    }
                    Err(e) => {
                        error!("Failed to validate score for task {}: {:?}", task.id, e);
                        // Continue processing other tasks instead of failing completely
                    }
                }
            }
        }
    }

    Ok(eval_result)
}

/// Spawn tasks without embedding support
pub async fn spawn_evaluation_tasks_without_embeddings(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
) -> JoinSet<LLMEvalTaskResult> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => LLMEvalTaskResult::success(record_id, workflow_result, None),
                Err(workflow_error) => LLMEvalTaskResult::failure(
                    record_id,
                    format!("Workflow execution failed: {}", workflow_error),
                    None,
                ),
            }
        });
    }

    join_set
}



/// Spawn tasks to run evaluation workflows with embedding calculations
/// # Arguments
/// * `workflow` - The workflow to execute for each record.
/// * `records` - The list of LLMEvalRecords to process.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `embedding_targets` - The list of keys in the record's context to generate embeddings.
/// # Returns
/// A JoinSet containing LLMEvalTaskResults for each record.
pub async fn spawn_evaluation_tasks_with_embeddings(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
    embedder: Arc<Embedder>,
    embedding_targets: Arc<Vec<String>>,
) -> JoinSet<LLMEvalTaskResult> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();
        let cloned_embedder = embedder.clone();
        let cloned_embedding_targets = embedding_targets.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();
            
            // Generate embeddings
            // We do this first because the workflow will consume the context
            let embeddings = match generate_embeddings_for_record(
                &record,
                &cloned_embedder,
                &cloned_embedding_targets,
            ).await {
                Ok(embeddings) => embeddings,
                Err(embedding_error) => {
                    warn!(
                        "Embedding generation failed for record {}: {}. Continuing with workflow execution.",
                        record_id, embedding_error
                    );
                    HashMap::new() // Continue with empty embeddings
                }
            };

            // Run workflow
            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => {
                    LLMEvalTaskResult::success(record_id, workflow_result, Some(embeddings))
                }
                Err(workflow_error) => {
                    LLMEvalTaskResult::failure(
                        record_id,
                        format!("Workflow execution failed: {}", workflow_error),
                        Some(embeddings),
                    )
                }
            }
        });
    }

    join_set
}

/// Helper for extracting embeddings for a single record. Used in the llm evaulation workflow.
/// # Arguments
/// * `record` - The LLMEvalRecord to extract embeddings from.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `embedding_targets` - The list of keys in the record's context to generate embeddings for.
/// # Returns
pub async fn generate_embeddings_for_record(
    record: &LLMEvalRecord,
    embedder: &Arc<Embedder>,
    embedding_targets: &[String],
) -> Result<HashMap<String, f32>, String> {
    let mut embeddings = HashMap::new();

    for target in embedding_targets {
        let texts = record
            .context
            .get(target)
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()]);

        if let Some(texts) = texts {
            match embedder.embed(texts).await {
                Ok(embedding_response) => match embedding_response.values() {
                    Ok(values) => {
                        if !values.is_empty() {
                            let mean = values.iter().sum::<f32>() / values.len() as f32;
                            embeddings.insert(target.clone(), mean);
                        } else {
                            warn!("Empty embedding values for target: {}", target);
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to extract embedding values for target {}: {:?}",
                            target, e
                        );
                        return Err(format!(
                            "Embedding extraction failed for target {}: {}",
                            target, e
                        ));
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to generate embedding for target {}: {:?}",
                        target, e
                    );
                    return Err(format!(
                        "Embedding generation failed for target {}: {}",
                        target, e
                    ));
                }
            }
        } else {
            warn!("No text found for embedding target: {}", target);
        }
    }

    Ok(embeddings)
}

/// Enhanced result collection with proper error handling
pub async fn collect_evaluation_results(
    mut join_set: JoinSet<LLMEvalTaskResult>,
) -> Result<LLMEvalResults, EvaluationError> {
    let mut eval_results = LLMEvalResults::new();
    let mut embedding_aggregates: HashMap<String, Vec<f32>> = HashMap::new();

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(task_result) => {
            
                // Collect embedding stats for aggregation
                if let Some(embeddings) = &task_result.embedding_results {
                    for (key, value) in embeddings {
                        embedding_aggregates
                            .entry(key.clone())
                            .or_insert_with(Vec::new)
                            .push(*value);
                    }
                }

                // Process workflow results
                // We need to handle both success and failure cases
                match (task_result.workflow_result, task_result.error) {

                    // Success case containing workflow result and no error
                    (Some(workflow_result), None) => {
                        // Success case
                        match process_workflow_result(workflow_result, task_result.id.clone()) {
                            Ok(eval_result) => {
                                eval_results.add_successful_result(task_result.id, eval_result);
                            }
                            Err(process_error) => {
                                eval_results.add_failed_result(
                                    task_result.id,
                                    format!("Result processing failed: {}", process_error),
                                );
                            }
                        }
                    }

                    // Failure case containing an error message
                    (None, Some(error)) => {
                        // Failure case
                        eval_results.add_failed_result(task_result.id, error);
                    }
                    _ => {
                        eval_results.add_failed_result(
                            task_result.id,
                            "Inconsistent task result state".to_string(),
                        );
                    }
                }
            }
            Err(join_error) => {
                error!("Task join error: {:?}", join_error);
                eval_results.add_failed_result(
                    "unknown".to_string(),
                    format!("Task execution failed: {}", join_error),
                );
            }
        }
    }

    // Calculate embedding statistics
    if !embedding_aggregates.is_empty() {
        let mut embedding_stats = HashMap::new();
        for (key, values) in embedding_aggregates {
            if !values.is_empty() {
                let mean = values.iter().sum::<f32>() / values.len() as f32;
                embedding_stats.insert(format!("{}_mean", key), mean);
            }
        }
        eval_results.embedding_stats = Some(embedding_stats);
    }

    Ok(eval_results)
}
