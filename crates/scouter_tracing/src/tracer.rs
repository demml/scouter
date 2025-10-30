use crate::error::TraceError;
use opentelemetry::{
    global::{self, BoxedSpan, BoxedTracer},
    trace::{Span, SpanKind, Tracer as OtelTracer, TracerProvider},
    KeyValue,
};
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, sync::RwLock};
use thiserror::Error;

#[pyclass]
#[derive(Clone)]
pub struct Tracer {
    tracer: Arc<BoxedTracer>,
    active_spans: Arc<RwLock<Vec<BoxedSpan>>>,
}

#[pymethods]
impl Tracer {
    #[new]
    #[pyo3(signature = (service_name, endpoint=None, sample_ratio=None))]
    pub fn new(
        service_name: String,
        endpoint: Option<String>,
        sample_ratio: Option<f64>,
    ) -> Result<Self, TraceError> {
        let tracer = Tracer::init_tracer(service_name, endpoint, sample_ratio)?;

        Ok(Tracer {
            tracer: Arc::new(tracer),
            active_spans: Arc::new(RwLock::new(Vec::new())),
        })
    }

    #[pyo3(signature = (name, attributes = None))]
    pub fn start(&self, name: String, attributes: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
        let mut span = self.tracer.start(name);

        if let Some(attrs) = attributes {
            for (key, value) in attrs.iter() {
                let key_str = key.extract::<String>()?;
                let value_str = value.extract::<String>()?;
                span.set_attribute(KeyValue::new(key_str, value_str));
            }
        }

        self.active_spans.write().unwrap().push(span);

        Ok(())
    }

    #[pyo3(signature = (name, attributes = None))]
    pub fn add_event(
        &self,
        name: &str,
        attributes: Option<HashMap<String, String>>,
    ) -> PyResult<()> {
        if let Ok(mut spans) = self.active_spans.write() {
            if let Some(span) = spans.last_mut() {
                let attrs = attributes
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(k, v)| KeyValue::new(k, v))
                    .collect::<Vec<_>>();
                span.add_event(name.to_string(), attrs);
            }
        }
        Ok(())
    }
}

impl Tracer {
    fn init_tracer(
        service_name: String,
        endpoint: Option<String>,
        sample_ratio: Option<f64>,
    ) -> Result<BoxedTracer, TraceError> {
        // Create base resource for the service
        let resource = Resource::builder()
            .with_attributes(vec![KeyValue::new("service.name", service_name.clone())])
            .build();

        // Determine sampler
        let sampler = sample_ratio
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn);

        let provider = match endpoint {
            // OTLP endpoint provided - use remote exporter
            Some(endpoint_url) => {
                let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint_url)
                    .with_protocol(Protocol::HttpBinary)
                    .build()
                    .map_err(|e| {
                        TraceError::InitializationError(format!(
                            "Failed to build OTLP exporter: {}",
                            e
                        ))
                    })?;

                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_batch_exporter(otlp_exporter)
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
            // No endpoint - use stdout exporter for local development
            None => {
                let stdout_exporter = opentelemetry_stdout::SpanExporter::default();

                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_simple_exporter(stdout_exporter)
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
        };

        // Set global tracer provider and get tracer instance
        global::set_tracer_provider(provider);
        let tracer = global::tracer(service_name);

        Ok(tracer)
    }
}
