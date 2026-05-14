use serde::Serialize;
use std::str::FromStr;
use tracing::warn;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum OtelProtocol {
    #[default]
    Grpc,
}

impl FromStr for OtelProtocol {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "grpc" => Ok(Self::Grpc),
            other => Err(format!("unsupported OTLP protocol: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OtelSettings {
    pub enabled: bool,
    pub endpoint: String,
    pub protocol: OtelProtocol,
    pub service_name: String,
    pub sample_ratio: f64,
    pub export_timeout_secs: u64,
}

impl Default for OtelSettings {
    fn default() -> Self {
        let enabled = std::env::var("SCOUTER_OTEL_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);

        let endpoint = std::env::var("SCOUTER_OTEL_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string());

        let protocol = std::env::var("SCOUTER_OTEL_PROTOCOL")
            .ok()
            .and_then(|v| match v.parse::<OtelProtocol>() {
                Ok(protocol) => Some(protocol),
                Err(err) => {
                    warn!("{err}; falling back to grpc");
                    None
                }
            })
            .unwrap_or_default();

        let service_name = std::env::var("SCOUTER_OTEL_SERVICE_NAME")
            .unwrap_or_else(|_| "scouter-server".to_string());

        let sample_ratio = std::env::var("SCOUTER_OTEL_SAMPLE_RATIO")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);

        let export_timeout_secs = std::env::var("SCOUTER_OTEL_EXPORT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);

        Self {
            enabled,
            endpoint,
            protocol,
            service_name,
            sample_ratio,
            export_timeout_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OTEL_ENV: &[&str] = &[
        "SCOUTER_OTEL_ENABLED",
        "SCOUTER_OTEL_ENDPOINT",
        "SCOUTER_OTEL_PROTOCOL",
        "SCOUTER_OTEL_SERVICE_NAME",
        "SCOUTER_OTEL_SAMPLE_RATIO",
        "SCOUTER_OTEL_EXPORT_TIMEOUT_SECS",
    ];

    fn with_clean_env<F: FnOnce()>(f: F) {
        let prev: Vec<_> = OTEL_ENV
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect();
        for key in OTEL_ENV {
            unsafe {
                std::env::remove_var(key);
            }
        }

        f();

        for (key, value) in prev {
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn otel_settings_defaults() {
        with_clean_env(|| {
            let settings = OtelSettings::default();
            assert!(!settings.enabled);
            assert_eq!(settings.endpoint, "http://localhost:4317");
            assert_eq!(settings.protocol, OtelProtocol::Grpc);
            assert_eq!(settings.service_name, "scouter-server");
            assert_eq!(settings.sample_ratio, 1.0);
            assert_eq!(settings.export_timeout_secs, 10);
        });
    }

    #[test]
    fn otel_settings_read_env_overrides() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("SCOUTER_OTEL_ENABLED", "true");
                std::env::set_var("SCOUTER_OTEL_ENDPOINT", "http://collector:4317");
                std::env::set_var("SCOUTER_OTEL_PROTOCOL", "grpc");
                std::env::set_var("SCOUTER_OTEL_SERVICE_NAME", "scouter-test");
                std::env::set_var("SCOUTER_OTEL_SAMPLE_RATIO", "0.25");
                std::env::set_var("SCOUTER_OTEL_EXPORT_TIMEOUT_SECS", "3");
            }

            let settings = OtelSettings::default();
            assert!(settings.enabled);
            assert_eq!(settings.endpoint, "http://collector:4317");
            assert_eq!(settings.protocol, OtelProtocol::Grpc);
            assert_eq!(settings.service_name, "scouter-test");
            assert_eq!(settings.sample_ratio, 0.25);
            assert_eq!(settings.export_timeout_secs, 3);
        });
    }

    #[test]
    fn sample_ratio_is_clamped_to_valid_range() {
        with_clean_env(|| {
            unsafe {
                std::env::set_var("SCOUTER_OTEL_SAMPLE_RATIO", "2.0");
            }

            assert_eq!(OtelSettings::default().sample_ratio, 1.0);
        });
    }
}
