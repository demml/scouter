use crate::error::TraceError;
use crate::exporter::scouter::ScouterSpanExporter;
use opentelemetry::KeyValue;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_sdk::Resource;
use prost::Message;
use pyo3::{prelude::*, types::PyTuple};
use scouter_events::queue::types::TransportConfig;
use scouter_settings::HttpConfig;
use scouter_types::{SCOUTER_SCOPE, SCOUTER_SCOPE_DEFAULT};
use std::sync::Arc;

/// Python-facing OTLP-compatible exporter that delegates to both
/// ScouterSpanExporter (for internal observability) and optionally
/// a remote OTLP collector
#[pyclass(name = "ScouterSpanExporter")]
pub struct PyScouterSpanExporter {
    _scouter_exporter: Arc<ScouterSpanExporter>,
    _resource: Resource,
    encode_module: Py<PyModule>,
}

#[pymethods]
impl PyScouterSpanExporter {
    /// Create a new ScouterSpanExporter that optionally forwards to an OTLP endpoint
    ///
    /// # Arguments
    /// * `transport_config` - Scouter internal transport configuration
    /// * `resource` - OpenTelemetry resource attributes
    /// * `endpoint` - Optional OTLP endpoint URL (e.g., "http://localhost:4317/v1/traces")
    /// * `headers` - Optional HTTP headers for OTLP requests (e.g., auth tokens)
    /// * `timeout_seconds` - Optional timeout for OTLP requests (default: 10)
    #[new]
    #[pyo3(signature = (
        service_name="scouter_service".to_string(),
        scope=SCOUTER_SCOPE_DEFAULT.to_string(),
        transport_config = None,
    ))]
    pub fn new(
        py: Python<'_>,
        service_name: String,
        scope: String,
        transport_config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // Convert Python objects to Rust types
        let transport_config = match transport_config {
            Some(config) => TransportConfig::from_py_config(config)?,
            None => {
                let config = HttpConfig::default();
                TransportConfig::Http(config)
            }
        };

        let resource = Resource::builder()
            .with_service_name(service_name.clone())
            .with_attributes([KeyValue::new(SCOUTER_SCOPE, scope.clone())])
            .build();

        // Create internal Scouter exporter
        let scouter_exporter = ScouterSpanExporter::new(transport_config, &resource)?;

        // import from opentelemetry.exporter.otlp.proto.common.trace_encoder
        let module = py.import("opentelemetry.exporter.otlp.proto.common.trace_encoder")?;

        Ok(PyScouterSpanExporter {
            _scouter_exporter: Arc::new(scouter_exporter),
            _resource: resource,
            encode_module: module.into(),
        })
    }

    #[pyo3(name = "export")]
    pub fn py_export(&self, py: Python<'_>, spans: Bound<'_, PyTuple>) -> Result<(), TraceError> {
        // import encode_spans
        let encode_spans = self
            .encode_module
            .bind(py)
            .getattr("encode_spans")
            .map_err(|e| TraceError::ExportError(format!("Failed to get encode_spans: {}", e)))?;

        // call serialized_data = encode_spans(spans).SerializePartialToString()
        let serialized_data = encode_spans
            .call1((spans,))
            .and_then(|encoded| encoded.call_method0("SerializePartialToString"))
            .map_err(|e| TraceError::ExportError(format!("Failed to encode spans: {}", e)))?;

        // Extract bytes from Python bytes object
        let bytes: &[u8] = serialized_data
            .extract()
            .map_err(|e| TraceError::ExportError(format!("Failed to extract bytes: {}", e)))?;

        // Deserialize into Rust ExportTraceServiceRequest
        let export_request = ExportTraceServiceRequest::decode(bytes)
            .map_err(|e| TraceError::ExportError(format!("Failed to decode protobuf: {}", e)))?;

        println!(
            "Exporting {} spans to Scouter",
            serde_json::to_string(&export_request).unwrap_or_else(|_| "unknown".to_string())
        );

        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<(), TraceError> {
        // Delegated shutdown
        println!("shutdown called");
        Ok(())
    }

    pub fn force_flush(&self) -> Result<(), TraceError> {
        // No-op for now
        println!("force_flush called");
        Ok(())
    }
}
