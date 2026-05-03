//! OTel-compliant Resource configuration for scouter.
//!
//! Resolution precedence per OTel SDK spec:
//!   explicit args > OTEL_SERVICE_NAME > OTEL_RESOURCE_ATTRIBUTES > defaults
//!
//! `OTEL_RESOURCE_ATTRIBUTES` parses as comma-separated `key=value`, with
//! URL-decoded values, per https://opentelemetry.io/docs/specs/otel/resource/sdk/

use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use pyo3::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

const ENV_OTEL_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";
const ENV_OTEL_RESOURCE_ATTRIBUTES: &str = "OTEL_RESOURCE_ATTRIBUTES";

const ATTR_SERVICE_NAME: &str = "service.name";
const ATTR_SERVICE_VERSION: &str = "service.version";
const ATTR_SERVICE_NAMESPACE: &str = "service.namespace";
const ATTR_SERVICE_INSTANCE_ID: &str = "service.instance.id";

const DEFAULT_SERVICE_NAME: &str = "unknown_service";

/// Process-wide OTel Resource configuration.
///
/// Build with explicit fields, env vars, or both. `build_resource()` resolves
/// per OTel SDK spec precedence.
#[pyclass(from_py_object)]
#[derive(Debug, Clone, Default)]
pub struct ScouterResourceConfig {
    #[pyo3(get, set)]
    pub service_name: Option<String>,
    #[pyo3(get, set)]
    pub service_version: Option<String>,
    #[pyo3(get, set)]
    pub service_namespace: Option<String>,
    #[pyo3(get, set)]
    pub service_instance_id: Option<String>,
    #[pyo3(get, set)]
    pub extra_attributes: HashMap<String, String>,
}

#[pymethods]
impl ScouterResourceConfig {
    #[new]
    #[pyo3(signature = (
        service_name = None,
        service_version = None,
        service_namespace = None,
        service_instance_id = None,
        extra_attributes = None,
    ))]
    fn py_new(
        service_name: Option<String>,
        service_version: Option<String>,
        service_namespace: Option<String>,
        service_instance_id: Option<String>,
        extra_attributes: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            service_name,
            service_version,
            service_namespace,
            service_instance_id,
            extra_attributes: extra_attributes.unwrap_or_default(),
        }
    }
}

impl ScouterResourceConfig {
    /// Build the OTel Resource. Pure function — no global state, no I/O
    /// other than env-var reads.
    pub fn build_resource(self) -> Resource {
        let env_attrs = parse_otel_resource_attributes();

        let service_name = self
            .service_name
            .or_else(|| std::env::var(ENV_OTEL_SERVICE_NAME).ok())
            .or_else(|| env_attrs.get(ATTR_SERVICE_NAME).cloned())
            .unwrap_or_else(|| DEFAULT_SERVICE_NAME.to_string());

        let service_version = self
            .service_version
            .or_else(|| env_attrs.get(ATTR_SERVICE_VERSION).cloned());

        let service_namespace = self
            .service_namespace
            .or_else(|| env_attrs.get(ATTR_SERVICE_NAMESPACE).cloned());

        // Spec: service.instance.id should default to a UUIDv4 when unset.
        let service_instance_id = self
            .service_instance_id
            .or_else(|| env_attrs.get(ATTR_SERVICE_INSTANCE_ID).cloned())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut attrs: Vec<KeyValue> =
            Vec::with_capacity(4 + self.extra_attributes.len() + env_attrs.len());

        if let Some(v) = service_version {
            attrs.push(KeyValue::new(ATTR_SERVICE_VERSION, v));
        }
        if let Some(v) = service_namespace {
            attrs.push(KeyValue::new(ATTR_SERVICE_NAMESPACE, v));
        }
        attrs.push(KeyValue::new(ATTR_SERVICE_INSTANCE_ID, service_instance_id));

        // Carry env-provided extras (other than the four well-known service.* keys
        // already handled above).
        for (k, v) in &env_attrs {
            if matches!(
                k.as_str(),
                ATTR_SERVICE_NAME
                    | ATTR_SERVICE_VERSION
                    | ATTR_SERVICE_NAMESPACE
                    | ATTR_SERVICE_INSTANCE_ID
            ) {
                continue;
            }
            // Explicit `extra_attributes` override env values.
            if !self.extra_attributes.contains_key(k) {
                attrs.push(KeyValue::new(k.clone(), v.clone()));
            }
        }
        for (k, v) in self.extra_attributes {
            attrs.push(KeyValue::new(k, v));
        }

        Resource::builder()
            .with_service_name(service_name)
            .with_attributes(attrs)
            .build()
    }
}

/// Parse `OTEL_RESOURCE_ATTRIBUTES` per spec: comma-separated `key=value`
/// pairs, with URL-decoded values. Whitespace around keys/values is trimmed.
/// Malformed pairs are silently skipped.
pub(crate) fn parse_otel_resource_attributes() -> HashMap<String, String> {
    let raw = match std::env::var(ENV_OTEL_RESOURCE_ATTRIBUTES) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };

    raw.split(',')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            let k = k.trim().to_string();
            if k.is_empty() {
                return None;
            }
            let v = urlencoding::decode(v.trim()).ok()?.into_owned();
            Some((k, v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        // Snapshot + restore — tests run with --test-threads=1 per AGENTS.md.
        let prev: Vec<_> = vars
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in vars {
            match v {
                // SAFETY: these tests run with `--test-threads=1`, so env mutation is serialized.
                Some(val) => unsafe { std::env::set_var(k, val) },
                // SAFETY: these tests run with `--test-threads=1`, so env mutation is serialized.
                None => unsafe { std::env::remove_var(k) },
            }
        }
        f();
        for (k, v) in prev {
            match v {
                // SAFETY: these tests run with `--test-threads=1`, so env mutation is serialized.
                Some(val) => unsafe { std::env::set_var(k, val) },
                // SAFETY: these tests run with `--test-threads=1`, so env mutation is serialized.
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }

    fn service_name(r: &Resource) -> String {
        r.get(&opentelemetry::Key::from_static_str("service.name"))
            .map(|v| v.to_string())
            .unwrap_or_default()
    }

    #[test]
    fn explicit_wins_over_env() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, Some("env-svc")),
                (ENV_OTEL_RESOURCE_ATTRIBUTES, Some("service.name=attr-svc")),
            ],
            || {
                let cfg = ScouterResourceConfig {
                    service_name: Some("explicit".into()),
                    ..Default::default()
                };
                assert_eq!(service_name(&cfg.build_resource()), "explicit");
            },
        );
    }

    #[test]
    fn env_service_name_wins_over_resource_attrs() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, Some("env-svc")),
                (ENV_OTEL_RESOURCE_ATTRIBUTES, Some("service.name=attr-svc")),
            ],
            || {
                let cfg = ScouterResourceConfig::default();
                assert_eq!(service_name(&cfg.build_resource()), "env-svc");
            },
        );
    }

    #[test]
    fn resource_attrs_used_when_no_explicit_or_env_service_name() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, None),
                (ENV_OTEL_RESOURCE_ATTRIBUTES, Some("service.name=attr-svc")),
            ],
            || {
                let cfg = ScouterResourceConfig::default();
                assert_eq!(service_name(&cfg.build_resource()), "attr-svc");
            },
        );
    }

    #[test]
    fn defaults_to_unknown_service() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, None),
                (ENV_OTEL_RESOURCE_ATTRIBUTES, None),
            ],
            || {
                let cfg = ScouterResourceConfig::default();
                assert_eq!(service_name(&cfg.build_resource()), "unknown_service");
            },
        );
    }

    #[test]
    fn instance_id_defaults_to_uuid() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, Some("svc")),
                (ENV_OTEL_RESOURCE_ATTRIBUTES, None),
            ],
            || {
                let r = ScouterResourceConfig::default().build_resource();
                let id = r
                    .get(&opentelemetry::Key::from_static_str("service.instance.id"))
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                assert!(Uuid::parse_str(&id).is_ok(), "got: {id}");
            },
        );
    }

    #[test]
    fn url_decode_in_resource_attrs() {
        with_env(
            &[
                (ENV_OTEL_SERVICE_NAME, None),
                (
                    ENV_OTEL_RESOURCE_ATTRIBUTES,
                    Some("k=hello%20world,service.name=acme%3Aagent"),
                ),
            ],
            || {
                let r = ScouterResourceConfig::default().build_resource();
                assert_eq!(service_name(&r), "acme:agent");
                let v = r
                    .get(&opentelemetry::Key::from_static_str("k"))
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                assert_eq!(v, "hello world");
            },
        );
    }
}
