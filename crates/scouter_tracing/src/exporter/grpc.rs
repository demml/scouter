use crate::error::TraceError;
use crate::exporter::traits::SpanExporterBuilder;
use crate::exporter::ExporterType;
use crate::utils::{ExportConfig, GrpcConfig, OtelProtocol};
use opentelemetry_otlp::ExportConfig as OtlpExportConfig;
use opentelemetry_otlp::SpanExporter as OtlpSpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::Resource;
use pyo3::prelude::*;
use scouter_types::{CompressionType, PyHelperFuncs};
use serde::Serialize;
use std::time::Duration;
#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct GrpcSpanExporter {
    #[pyo3(get)]
    pub sample_ratio: Option<f64>,

    #[pyo3(get)]
    pub batch_export: bool,

    #[pyo3(get)]
    endpoint: Option<String>,

    #[pyo3(get)]
    protocol: OtelProtocol,

    #[pyo3(get)]
    timeout: Option<u64>,

    #[pyo3(get)]
    compression: Option<CompressionType>,
}

#[pymethods]
impl GrpcSpanExporter {
    #[new]
    #[pyo3(signature = (batch_export=true, export_config=None, grpc_config=None, sample_ratio=None))]
    pub fn new(
        batch_export: bool,
        export_config: Option<&ExportConfig>,
        grpc_config: Option<&GrpcConfig>,
        sample_ratio: Option<f64>,
    ) -> Result<Self, TraceError> {
        let (endpoint, protocol, timeout) = if let Some(config) = export_config {
            (
                config.endpoint.clone(),
                config.protocol.clone(),
                config.timeout,
            )
        } else {
            (None, OtelProtocol::default(), None)
        };

        let compression = if let Some(grpc_config) = grpc_config {
            grpc_config.compression.clone()
        } else {
            None
        };

        Ok(Self {
            batch_export,
            sample_ratio,
            endpoint,
            protocol,
            timeout,
            compression,
        })
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl SpanExporterBuilder for GrpcSpanExporter {
    type Exporter = OtlpSpanExporter;

    fn export_type(&self) -> ExporterType {
        ExporterType::Grpc
    }

    fn sample_ratio(&self) -> Option<f64> {
        self.sample_ratio
    }

    fn batch_export(&self) -> bool {
        self.batch_export
    }

    fn build_exporter(&self, resource: &Resource) -> Result<Self::Exporter, TraceError> {
        // Reconstruct the OtlpExportConfig each time
        let timeout = self.timeout.map(Duration::from_secs);
        let export_config = OtlpExportConfig {
            endpoint: self.endpoint.clone(),
            protocol: self.protocol.to_otel_protocol(),
            timeout,
        };

        let mut builder = opentelemetry_otlp::SpanExporter::builder().with_tonic();

        if let Some(compression) = &self.compression {
            builder = builder.with_compression(compression.to_otel_compression()?);
        }

        builder = builder.with_export_config(export_config);
        let mut exporter = builder.build()?;
        exporter.set_resource(resource);

        Ok(exporter)
    }
}
