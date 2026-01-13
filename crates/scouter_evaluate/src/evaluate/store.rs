use chrono::{DateTime, Utc};
use scouter_types::genai::AssertionResult;
use std::sync::RwLock;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Assertion,
    LLMJudge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskMetadata {
    pub task_type: TaskType,
    pub is_conditional: bool,
}

/// Registry that tracks task IDs and their types for store routing
#[derive(Debug)]
pub struct TaskRegistry {
    /// Maps task_id -> TaskType
    registry: HashMap<String, TaskMetadata>,
    dependency_map: HashMap<String, Arc<Vec<String>>>,
    skipped_tasks: RwLock<HashSet<String>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        debug!("Initializing TaskRegistry");
        Self {
            registry: HashMap::new(),
            dependency_map: HashMap::new(),
            skipped_tasks: RwLock::new(HashSet::new()),
        }
    }

    pub fn mark_skipped(&self, task_id: String) {
        self.skipped_tasks.write().unwrap().insert(task_id);
    }

    pub fn is_skipped(&self, task_id: &str) -> bool {
        self.skipped_tasks.read().unwrap().contains(task_id)
    }

    /// Register a task with its type and conditional status
    pub fn register(&mut self, task_id: String, task_type: TaskType, is_conditional: bool) {
        self.registry.insert(
            task_id,
            TaskMetadata {
                task_type,
                is_conditional,
            },
        );
    }

    /// Get the type of a task by ID
    pub fn get_type(&self, task_id: &str) -> Option<TaskType> {
        self.registry.get(task_id).map(|meta| meta.task_type)
    }

    /// Check if a task is marked as conditional
    pub fn is_conditional(&self, task_id: &str) -> bool {
        self.registry
            .get(task_id)
            .map(|meta| meta.is_conditional)
            .unwrap_or(false)
    }

    /// Check if a task is registered
    pub fn contains(&self, task_id: &str) -> bool {
        self.registry.contains_key(task_id)
    }

    /// Register dependencies for a task
    /// # Arguments
    /// * `task_id` - The ID of the task
    /// * `dependencies` - A list of task IDs that this task depends on
    pub fn register_dependencies(&mut self, task_id: String, dependencies: Vec<String>) {
        self.dependency_map.insert(task_id, Arc::new(dependencies));
    }

    /// For a given task ID, get its dependencies
    /// # Arguments
    /// * `task_id` - The ID of the task
    pub fn get_dependencies(&self, task_id: &str) -> Option<Arc<Vec<String>>> {
        if !self.registry.contains_key(task_id) {
            return None;
        }

        Some(self.dependency_map.get(task_id)?.clone())
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// start time, end time, result
type AssertionResultType = (DateTime<Utc>, DateTime<Utc>, AssertionResult);

/// Store for assertion results
#[derive(Debug)]
pub struct AssertionResultStore {
    /// Internal store mapping task IDs to results
    store: HashMap<String, AssertionResultType>,
}

impl AssertionResultStore {
    pub fn new() -> Self {
        AssertionResultStore {
            store: HashMap::new(),
        }
    }

    pub fn store(
        &mut self,
        task_id: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        result: AssertionResult,
    ) {
        self.store.insert(task_id, (start_time, end_time, result));
    }

    pub fn retrieve(&self, task_id: &str) -> Option<AssertionResultType> {
        self.store.get(task_id).cloned()
    }
}

impl Default for AssertionResultStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Store for raw LLM responses
#[derive(Debug)]
pub struct LLMResponseStore {
    /// Internal store mapping task IDs to raw LLM responses
    store: HashMap<String, serde_json::Value>,
}

impl LLMResponseStore {
    pub fn new() -> Self {
        LLMResponseStore {
            store: HashMap::new(),
        }
    }

    pub fn store(&mut self, task_id: String, response: serde_json::Value) {
        self.store.insert(task_id, response);
    }

    pub fn retrieve(&self, task_id: &str) -> Option<serde_json::Value> {
        self.store.get(task_id).cloned()
    }
}

impl Default for LLMResponseStore {
    fn default() -> Self {
        Self::new()
    }
}

// create a very simple test
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_task_registry() {
        let mut registry = TaskRegistry::new();
        registry.register("task1".to_string(), TaskType::Assertion, false);
        registry.register("task2".to_string(), TaskType::LLMJudge, false);
        assert_eq!(registry.get_type("task1"), Some(TaskType::Assertion));
        assert_eq!(registry.get_type("task2"), Some(TaskType::LLMJudge));
        assert!(registry.contains("task1"));
        assert!(!registry.contains("task3"));
    }
}
