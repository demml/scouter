use crate::error::EvaluationError;
use crate::types::{EvaluationConfig, LLMEvalRecord, LLMEvalResults, LLMEvalTaskResult};
use itertools::iproduct;
use linfa::{traits::*, Dataset};
use linfa_clustering::Dbscan;
use linfa_reduction::Pca;
use ndarray::{Array1, Array2};
use num_traits::FromPrimitive;
use potato_head::{
    Embedder, EmbeddingInput, PyEmbedder, Score, StructuredOutput, TaskStatus, Workflow,
    WorkflowError,
};
use pyo3::prelude::*;
use rayon::prelude::*;
use regex::Regex;
use serde_json::Value;
use simsimd::SpatialSimilarity;
use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use tokio::task::JoinSet;
use tracing::{error, warn};

const REGEX_FIELD_PARSE_PATTERN: &str = r"[a-zA-Z_][a-zA-Z0-9_]*|\[[0-9]+\]";

/// Process a workflow result and extract scores from completed tasks
pub fn process_workflow_result(
    workflow_result: Arc<RwLock<Workflow>>,
) -> Result<BTreeMap<String, Score>, EvaluationError> {
    let mut metrics = BTreeMap::new();

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
                        metrics.insert(task.id.clone(), score);
                    }
                    Err(e) => {
                        error!("Failed to validate score for task {}: {:?}", task.id, e);
                        // Continue processing other tasks instead of failing completely
                    }
                }
            }
        }
    }

    Ok(metrics)
}

/// Spawn tasks without embedding support
/// This function will spawn a task that runs the workflows and extracts the results
/// If there is an error during workflow execution, it will log the error and return None for that record
/// # Arguments
/// * `workflow` - The workflow to execute for each record.
/// * `records` - The list of LLMEvalRecords to process.
/// # Returns
/// A JoinSet containing tuples of record ID and optional LLMEvalTaskResult.
pub async fn spawn_evaluation_tasks_without_embeddings(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
) -> JoinSet<(String, Option<LLMEvalTaskResult>)> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => {
                    // parse the workflow result
                    match process_workflow_result(workflow_result) {
                        Ok(metrics) => (
                            record_id.clone(),
                            Some(LLMEvalTaskResult::new(record_id, metrics, BTreeMap::new())),
                        ),
                        Err(error) => {
                            error!(
                                "Failed to process workflow result for record {}: {}",
                                record_id, error
                            );
                            (record_id, None)
                        }
                    }
                }
                Err(workflow_error) => {
                    error!(
                        "Workflow execution failed for record {}: {}",
                        record_id, workflow_error
                    );
                    (record_id, None)
                }
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
    config: &Arc<EvaluationConfig>,
) -> JoinSet<(String, Option<LLMEvalTaskResult>)> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();
        let cloned_embedder = embedder.clone();
        let cloned_config = config.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            // Generate embeddings
            // We do this first because the workflow will consume the context
            let embeddings = generate_embeddings_for_record(
                &record,
                &cloned_embedder,
                &cloned_config.embedding_targets,
            )
            .await;

            // Run workflow
            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => {
                    // parse the workflow result
                    match process_workflow_result(workflow_result) {
                        Ok(metrics) => (
                            record_id.clone(),
                            Some(LLMEvalTaskResult::new(record_id, metrics, embeddings)),
                        ),
                        Err(error) => {
                            error!(
                                "Failed to process workflow result for record {}: {}",
                                record_id, error
                            );
                            (record_id, None)
                        }
                    }
                }
                Err(workflow_error) => {
                    error!(
                        "Workflow execution failed for record {}: {}",
                        record_id, workflow_error
                    );
                    (record_id, None)
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
) -> BTreeMap<String, Vec<f32>> {
    let mut embeddings = BTreeMap::new();

    for target in embedding_targets {
        let texts = record
            .context
            .get(target)
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()]);

        if let Some(texts) = texts {
            match embedder.embed(EmbeddingInput::Texts(texts)).await {
                Ok(embedding_response) => match embedding_response.values() {
                    Ok(values) => {
                        // move ownership of values into Embedding struct
                        embeddings.insert(target.clone(), values.to_vec());
                    }
                    Err(e) => {
                        error!(
                            "Failed to extract embedding values for target {}: {:?}",
                            target, e
                        );
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to generate embedding for target {}: {:?}",
                        target, e
                    );
                }
            }
        } else {
            warn!("No text found for embedding target: {}", target);
        }
    }

    embeddings
}

/// Enhanced result collection with proper error handling
pub async fn collect_evaluation_results(
    mut join_set: JoinSet<(String, Option<LLMEvalTaskResult>)>,
) -> Result<LLMEvalResults, EvaluationError> {
    let mut eval_results = LLMEvalResults::new();

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(task_result) => {
                let (record_id, task_result_opt) = task_result;
                if let Some(task_result) = task_result_opt {
                    eval_results.results.entry(record_id).or_insert(task_result);
                } else {
                    eval_results.errored_tasks.push(record_id);
                }
            }
            Err(join_error) => {
                error!("Task join error: {:?}", join_error);
                // We can't associate this error with a specific record ID
            }
        }
    }

    Ok(eval_results)
}

/// Helper function for extracting embedder and runtime from optional PyEmbedder
/// # Arguments
/// * `embedder` - Optional reference to a PyEmbedder instance.
/// # Returns
/// An optional Arc-wrapped Embedder instance if provided, otherwise None.
pub fn parse_embedder(
    embedder: Option<&Bound<'_, PyAny>>,
) -> Result<Option<Arc<Embedder>>, EvaluationError> {
    // Extract embedder and runtime if PyEmbedder is provided
    let embedder_arc = if let Some(embedder_bound) = embedder {
        if embedder_bound.is_instance_of::<PyEmbedder>() {
            let py_embedder = embedder_bound.extract::<PyEmbedder>()?;
            Some(py_embedder.embedder.clone())
        } else {
            // embedder provided but not a PyEmbedder instance
            return Err(EvaluationError::InvalidEmbedderType);
        }
    } else {
        None
    };
    Ok(embedder_arc)
}

/// Calculate the mean of for a slice of f32 values
/// There's no need for a generic implementation here, as we only need f32 for embeddings
pub fn compute_mean(vec: &[f32]) -> Option<f64> {
    match vec.len() {
        0 => None,
        _ => {
            let sum = vec.iter().sum::<f32>();
            let length = f32::from_usize(vec.len())?;

            let mean = sum / length;
            Some(mean as f64)
        }
    }
}

/// Uses the DBSCAN algorithm to cluster the provided data
/// Returns an array where each element corresponds to the cluster ID of the respective data point
pub fn cluster(data: &Array2<f64>) -> Result<Array1<Option<usize>>, EvaluationError> {
    let min_points = 3;
    let clusters = Dbscan::params(min_points).transform(data)?;
    Ok(clusters)
}

pub fn compute_similarity(
    targets: &Vec<String>,
    embeddings: &BTreeMap<String, Vec<f32>>,
    scores: &mut BTreeMap<String, f64>,
) {
    for (a, b) in iproduct!(targets, targets) {
        // only want unique pairs
        if a == b {
            continue;
        }
        if let (Some(vec_a), Some(vec_b)) = (embeddings.get(a), embeddings.get(b)) {
            if vec_a.len() != vec_b.len() {
                warn!(
                    "Embedding length mismatch for targets {} and {}: {} vs {}",
                    a,
                    b,
                    vec_a.len(),
                    vec_b.len()
                );
                continue;
            }

            let similarity = f32::cosine(vec_a, vec_b).unwrap_or(-1.0);
            let key = format!("{}_{}_cosine", a, b);
            scores.insert(key, similarity);
        } else {
            warn!("Missing embeddings for targets {} or {}", a, b);
        }
    }
}

pub fn post_process(results: &mut LLMEvalResults, config: &Arc<EvaluationConfig>) {
    // compute means for each embedding target
    results.results.par_iter_mut().for_each(|(_, task_result)| {
        for (target, values) in task_result.embedding.iter() {
            let mean = compute_mean(values).unwrap_or(-1.0);
            task_result.mean_embeddings.insert(target.clone(), mean);
        }
        compute_similarity(
            &config.embedding_targets,
            &task_result.embedding,
            &mut task_result.similarity_scores,
        );
    });
}

/// Reduced the dimensionality of the data using PCA.
/// This is needed to visualize the clusters in 2D space.
/// # Arguments
/// * `data` - The input data to reduce.
/// * `cluster_ids` - The cluster IDs for each data point.
pub fn reduce_dimensions(
    data: &Array2<f64>,
    cluster_ids: &[i32],
) -> Result<Array2<f64>, EvaluationError> {
    let ds = Dataset::new(
        data.clone(),
        Array2::from_shape_vec(
            (data.nrows(), 1),
            cluster_ids.iter().map(|&x| x as f64).collect(),
        )
        .unwrap(),
    );

    let embedding = Pca::params(2).whiten(false).fit(&ds)?;

    let prediction = embedding.predict(&ds);
    Ok(prediction)
}

pub struct FieldEvaluator;

/// Utility for extracting field values from JSON-like structures
/// Supports: "field", "field.subfield", "field[0]", "field[0].subfield"
impl FieldEvaluator {
    pub fn extract_field_value(json: &Value, field_path: &str) -> Result<Value, EvaluationError> {
        let path_segments = Self::parse_field_path(field_path)?;
        let mut current_value = json;

        for segment in path_segments {
            current_value = match segment {
                PathSegment::Field(field_name) => current_value
                    .get(&field_name)
                    .ok_or_else(|| EvaluationError::FieldNotFound(field_name))?,
                PathSegment::Index(index) => current_value
                    .get(index)
                    .ok_or_else(|| EvaluationError::IndexNotFound(index))?,
            };
        }

        Ok(current_value.clone())
    }

    fn parse_field_path(path: &str) -> Result<Vec<PathSegment>, EvaluationError> {
        static PATH_REGEX: OnceLock<Regex> = OnceLock::new();
        let regex = PATH_REGEX.get_or_init(|| {
            Regex::new(REGEX_FIELD_PARSE_PATTERN)
                .expect("Invalid regex pattern in REGEX_FIELD_PARSE_PATTERN")
        });

        let mut segments = Vec::new();

        for capture in regex.find_iter(path) {
            let segment_str = capture.as_str();

            if segment_str.starts_with('[') && segment_str.ends_with(']') {
                // Array index: [0], [1], etc.
                let index_str = &segment_str[1..segment_str.len() - 1];
                let index: usize = index_str
                    .parse()
                    .map_err(|_| EvaluationError::InvalidArrayIndex(index_str.to_string()))?;
                segments.push(PathSegment::Index(index));
            } else {
                // Field name: field, subfield, etc.
                segments.push(PathSegment::Field(segment_str.to_string()));
            }
        }

        if segments.is_empty() {
            return Err(EvaluationError::EmptyFieldPath);
        }

        Ok(segments)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PathSegment {
    Field(String),
    Index(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Test data matching your StructuredTaskOutput example
    fn get_test_json() -> Value {
        json!({
            "tasks": ["task1", "task2", "task3"],
            "status": "in_progress",
            "metadata": {
                "created_by": "user_123",
                "priority": "high",
                "tags": ["urgent", "backend"],
                "nested": {
                    "deep": {
                        "value": "found_it"
                    }
                }
            },
            "counts": {
                "total": 42,
                "completed": 15
            },
            "empty_array": [],
            "single_item": ["only_one"]
        })
    }

    #[test]
    fn test_parse_field_path_simple_field() {
        let segments = FieldEvaluator::parse_field_path("status").unwrap();
        assert_eq!(segments, vec![PathSegment::Field("status".to_string())]);
    }

    #[test]
    fn test_parse_field_path_nested_field() {
        let segments = FieldEvaluator::parse_field_path("metadata.created_by").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("created_by".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_field_path_array_index() {
        let segments = FieldEvaluator::parse_field_path("tasks[0]").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("tasks".to_string()),
                PathSegment::Index(0)
            ]
        );
    }

    #[test]
    fn test_parse_field_path_complex() {
        let segments = FieldEvaluator::parse_field_path("metadata.tags[1]").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("tags".to_string()),
                PathSegment::Index(1)
            ]
        );
    }

    #[test]
    fn test_parse_field_path_deep_nested() {
        let segments = FieldEvaluator::parse_field_path("metadata.nested.deep.value").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("nested".to_string()),
                PathSegment::Field("deep".to_string()),
                PathSegment::Field("value".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_field_path_underscore_field() {
        let segments = FieldEvaluator::parse_field_path("created_by").unwrap();
        assert_eq!(segments, vec![PathSegment::Field("created_by".to_string())]);
    }

    #[test]
    fn test_parse_field_path_empty_string() {
        let result = FieldEvaluator::parse_field_path("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty field path"));
    }

    #[test]
    fn test_extract_simple_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "status").unwrap();
        assert_eq!(result, json!("in_progress"));
    }

    #[test]
    fn test_extract_array_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks").unwrap();
        assert_eq!(result, json!(["task1", "task2", "task3"]));
    }

    #[test]
    fn test_extract_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks[0]").unwrap();
        assert_eq!(result, json!("task1"));

        let result = FieldEvaluator::extract_field_value(&json, "tasks[2]").unwrap();
        assert_eq!(result, json!("task3"));
    }

    #[test]
    fn test_extract_nested_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.created_by").unwrap();
        assert_eq!(result, json!("user_123"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.priority").unwrap();
        assert_eq!(result, json!("high"));
    }

    #[test]
    fn test_extract_nested_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[0]").unwrap();
        assert_eq!(result, json!("urgent"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[1]").unwrap();
        assert_eq!(result, json!("backend"));
    }

    #[test]
    fn test_extract_deep_nested_field() {
        let json = get_test_json();
        let result =
            FieldEvaluator::extract_field_value(&json, "metadata.nested.deep.value").unwrap();
        assert_eq!(result, json!("found_it"));
    }

    #[test]
    fn test_extract_numeric_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "counts.total").unwrap();
        assert_eq!(result, json!(42));

        let result = FieldEvaluator::extract_field_value(&json, "counts.completed").unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn test_extract_empty_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "empty_array").unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_extract_single_item_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "single_item[0]").unwrap();
        assert_eq!(result, json!("only_one"));
    }

    #[test]
    fn test_extract_nonexistent_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Field 'nonexistent' not found"));
    }

    #[test]
    fn test_extract_nonexistent_nested_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Field 'nonexistent' not found"));
    }

    #[test]
    fn test_extract_array_index_out_of_bounds() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks[99]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Index 99 not found"));
    }

    #[test]
    fn test_extract_array_index_on_non_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "status[0]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Index 0 not found"));
    }

    #[test]
    fn test_extract_field_on_array_element() {
        let json = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]
        });

        let result = FieldEvaluator::extract_field_value(&json, "users[0].name").unwrap();
        assert_eq!(result, json!("Alice"));

        let result = FieldEvaluator::extract_field_value(&json, "users[1].age").unwrap();
        assert_eq!(result, json!(25));
    }

    #[test]
    fn test_structured_task_output_scenarios() {
        // Test scenarios based on your StructuredTaskOutput example
        let json = json!({
            "tasks": ["setup_database", "create_api", "write_tests"],
            "status": "in_progress"
        });

        // Test extracting the tasks array
        let tasks = FieldEvaluator::extract_field_value(&json, "tasks").unwrap();
        assert!(tasks.is_array());
        assert_eq!(tasks.as_array().unwrap().len(), 3);

        // Test extracting individual task items
        let first_task = FieldEvaluator::extract_field_value(&json, "tasks[0]").unwrap();
        assert_eq!(first_task, json!("setup_database"));

        // Test extracting status
        let status = FieldEvaluator::extract_field_value(&json, "status").unwrap();
        assert_eq!(status, json!("in_progress"));
    }

    #[test]
    fn test_real_world_llm_response_structure() {
        // Test with a more complex LLM response structure
        let json = json!({
            "analysis": {
                "sentiment": "positive",
                "confidence": 0.85,
                "keywords": ["innovation", "growth", "success"]
            },
            "recommendations": [
                {
                    "action": "increase_investment",
                    "priority": "high",
                    "estimated_impact": 0.75
                },
                {
                    "action": "expand_team",
                    "priority": "medium",
                    "estimated_impact": 0.60
                }
            ],
            "summary": "Overall positive outlook with strong growth potential"
        });

        // Test nested object extraction
        let sentiment = FieldEvaluator::extract_field_value(&json, "analysis.sentiment").unwrap();
        assert_eq!(sentiment, json!("positive"));

        // Test array of objects
        let first_action =
            FieldEvaluator::extract_field_value(&json, "recommendations[0].action").unwrap();
        assert_eq!(first_action, json!("increase_investment"));

        // Test numeric extraction
        let confidence = FieldEvaluator::extract_field_value(&json, "analysis.confidence").unwrap();
        assert_eq!(confidence, json!(0.85));

        // Test array element extraction
        let first_keyword =
            FieldEvaluator::extract_field_value(&json, "analysis.keywords[0]").unwrap();
        assert_eq!(first_keyword, json!("innovation"));
    }
}
