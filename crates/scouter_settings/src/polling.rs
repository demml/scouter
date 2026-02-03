use chrono::Duration;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PollingSettings {
    pub num_workers: usize,
    pub max_retries: usize,
}

impl Default for PollingSettings {
    fn default() -> Self {
        let num_workers = std::env::var("POLLING_WORKER_COUNT")
            .unwrap_or_else(|_| "4".to_string())
            .parse::<usize>()
            .unwrap();

        let max_retries = std::env::var("MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap();

        Self {
            num_workers,
            max_retries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenAIPollerSettings {
    pub max_retries: i32,
    pub genai_workers: usize,
    pub trace_wait_timeout: Duration,
    pub trace_backoff: Duration,
    pub trace_reschedule_delay: Duration,
}

impl Default for GenAIPollerSettings {
    fn default() -> Self {
        let genai_workers = std::env::var("GENAI_WORKER_COUNT")
            .unwrap_or_else(|_| "2".to_string())
            .parse::<usize>()
            .unwrap();

        let max_retries = std::env::var("GENAI_MAX_RETRIES")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<i32>()
            .unwrap();

        let trace_wait_timeout = Duration::seconds(
            std::env::var("GENAI_TRACE_WAIT_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".to_string())
                .parse::<i64>()
                .unwrap(),
        );

        let trace_backoff = Duration::milliseconds(
            std::env::var("GENAI_TRACE_BACKOFF_MILLIS")
                .unwrap_or_else(|_| "100".to_string())
                .parse::<i64>()
                .unwrap(),
        );

        let trace_reschedule_delay = Duration::seconds(
            std::env::var("GENAI_TRACE_RESCHEDULE_DELAY_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse::<i64>()
                .unwrap(),
        );

        Self {
            max_retries,
            trace_wait_timeout,
            trace_backoff,
            trace_reschedule_delay,
            genai_workers,
        }
    }
}
