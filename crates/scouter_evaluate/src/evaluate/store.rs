use scouter_types::genai::AssertionResult;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    Assertion,
    LLMJudge,
}

/// Registry that tracks task IDs and their types for store routing
#[derive(Debug)]
pub struct TaskRegistry {
    /// Maps task_id -> TaskType
    registry: HashMap<String, TaskType>,
    dependency_map: HashMap<String, Vec<String>>,
    first_level_tasks: Vec<String>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            dependency_map: HashMap::new(),
            first_level_tasks: Vec::new(),
        }
    }

    /// Register a task with its type
    pub fn register(&mut self, task_id: String, task_type: TaskType) {
        self.registry.insert(task_id, task_type);
    }

    /// Get the type of a task by ID
    pub fn get_type(&self, task_id: &str) -> Option<TaskType> {
        self.registry.get(task_id).copied()
    }

    /// Check if a task is registered
    pub fn contains(&self, task_id: &str) -> bool {
        self.registry.contains_key(task_id)
    }

    // set first level tasks
    pub fn set_first_level_tasks(&mut self, task_ids: Vec<String>) {
        self.first_level_tasks = task_ids;
    }

    /// Register dependencies for a task
    /// # Arguments
    /// * `task_id` - The ID of the task
    /// * `dependencies` - A list of task IDs that this task depends on
    pub fn register_dependencies(&mut self, task_id: String, dependencies: Vec<String>) {
        self.dependency_map.insert(task_id, dependencies);
    }

    /// For a given task ID, get its dependencies
    /// # Arguments
    /// * `task_id` - The ID of the task
    pub fn get_dependencies(&self, task_id: &str) -> Option<Vec<&str>> {
        if !self.registry.contains_key(task_id) {
            return None;
        }

        let mut conditions = Vec::new();
        if let Some(dependencies) = self.dependency_map.get(task_id) {
            for dep_id in dependencies {
                if self.registry.contains_key(dep_id.as_str()) {
                    conditions.push(dep_id.as_str());
                }
            }
        }

        Some(conditions)
    }

    pub fn is_first_level_task(&self, task_id: &str) -> bool {
        self.first_level_tasks.contains(&task_id.to_string())
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Store for assertion results
#[derive(Debug)]
pub struct AssertionResultStore {
    /// Internal store mapping task IDs to results
    store: HashMap<String, AssertionResult>,
}

impl AssertionResultStore {
    pub fn new() -> Self {
        AssertionResultStore {
            store: HashMap::new(),
        }
    }

    pub fn store(&mut self, task_id: String, result: AssertionResult) {
        self.store.insert(task_id, result);
    }

    pub fn retrieve(&self, task_id: &str) -> Option<AssertionResult> {
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
        registry.register("task1".to_string(), TaskType::Assertion);
        registry.register("task2".to_string(), TaskType::LLMJudge);
        assert_eq!(registry.get_type("task1"), Some(TaskType::Assertion));
        assert_eq!(registry.get_type("task2"), Some(TaskType::LLMJudge));
        assert!(registry.contains("task1"));
        assert!(!registry.contains("task3"));
    }
}
