use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PollingSettings {
    pub num_workers: usize,
    pub genai_workers: usize,
    pub max_retries: usize,
}

impl Default for PollingSettings {
    fn default() -> Self {
        let num_workers = std::env::var("POLLING_WORKER_COUNT")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .unwrap();

        let genai_workers = std::env::var("LLM_WORKER_COUNT")
            .unwrap_or_else(|_| "2".to_string())
            .parse::<usize>()
            .unwrap();

        let max_retries = std::env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap();

        Self {
            num_workers,
            genai_workers,
            max_retries,
        }
    }
}
