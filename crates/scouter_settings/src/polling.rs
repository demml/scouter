use chrono::Duration;
use serde::Deserialize;
use serde::Serialize;

use crate::storage::trace_refresh_interval_secs_from_env;

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
pub struct AgentPollerSettings {
    pub max_retries: i32,
    pub genai_workers: usize,
    pub trace_wait_timeout: Duration,
    pub trace_backoff: Duration,
    pub trace_reschedule_delay: Duration,
    pub trace_visibility_buffer: Duration,
}

impl Default for AgentPollerSettings {
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

        let trace_visibility_buffer = Self::trace_visibility_buffer();

        Self {
            max_retries,
            trace_wait_timeout,
            trace_backoff,
            trace_reschedule_delay,
            trace_visibility_buffer,
            genai_workers,
        }
    }
}

impl AgentPollerSettings {
    pub fn trace_visibility_buffer() -> Duration {
        let refresh_secs = trace_refresh_interval_secs_from_env() as i64;
        let min_floor = refresh_secs + 2;

        let configured = std::env::var("SCOUTER_TRACE_VISIBILITY_BUFFER_SECS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(min_floor)
            .max(0);

        if configured < min_floor {
            panic!(
                "SCOUTER_TRACE_VISIBILITY_BUFFER_SECS={} is below the safe minimum {} \
                 (SCOUTER_TRACE_REFRESH_INTERVAL_SECS + 2). A smaller buffer can cause \
                 the eval poller to fetch spans before the local Delta snapshot sees the anchor.",
                configured, min_floor
            );
        }

        Duration::seconds(configured)
    }
}

#[cfg(test)]
mod tests {
    use super::AgentPollerSettings;

    #[test]
    fn trace_visibility_buffer_floor() {
        unsafe {
            std::env::set_var("SCOUTER_TRACE_REFRESH_INTERVAL_SECS", "10");
            std::env::set_var("SCOUTER_TRACE_VISIBILITY_BUFFER_SECS", "1");
        }
        let result = std::panic::catch_unwind(AgentPollerSettings::trace_visibility_buffer);
        unsafe {
            std::env::remove_var("SCOUTER_TRACE_REFRESH_INTERVAL_SECS");
            std::env::remove_var("SCOUTER_TRACE_VISIBILITY_BUFFER_SECS");
        }
        assert!(result.is_err());
    }

    #[test]
    fn trace_visibility_buffer_ignores_negative_refresh_like_storage_settings() {
        unsafe {
            std::env::set_var("SCOUTER_TRACE_REFRESH_INTERVAL_SECS", "-10");
            std::env::set_var("SCOUTER_TRACE_VISIBILITY_BUFFER_SECS", "1");
        }
        let result = std::panic::catch_unwind(AgentPollerSettings::trace_visibility_buffer);
        unsafe {
            std::env::remove_var("SCOUTER_TRACE_REFRESH_INTERVAL_SECS");
            std::env::remove_var("SCOUTER_TRACE_VISIBILITY_BUFFER_SECS");
        }
        assert!(result.is_err());
    }
}
