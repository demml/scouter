use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PollingSettings {
    pub num_workers: usize,
}

impl Default for PollingSettings {
    fn default() -> Self {
        let num_workers = std::env::var("POLLING_WORKER_COUNT")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .unwrap();

        Self { num_workers }
    }
}
