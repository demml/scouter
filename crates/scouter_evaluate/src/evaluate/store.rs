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
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
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
