use std::collections::HashMap;

use crate::error::TraceError;
use crate::exporter::traits::SpanExporterBuilder;
use crate::utils::{ExportConfig, HttpConfig, Protocol};
use opentelemetry_otlp::ExportConfig as OtlpExportConfig;
use opentelemetry_otlp::SpanExporter as OtlpSpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use pyo3::prelude::*;
use scouter_types::{CompressionType, PyHelperFuncs};
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct HttpSpanExporter {
    #[pyo3(get)]
    pub sample_ratio: Option<f64>,

    #[pyo3(get)]
    pub batch_export: bool,

    #[pyo3(get)]
    endpoint: Option<String>,

    #[pyo3(get)]
    protocol: Protocol,

    #[pyo3(get)]
    timeout: Option<u64>,

    #[pyo3(get)]
    headers: Option<HashMap<String, String>>,

    #[pyo3(get)]
    compression: Option<CompressionType>,
}

#[pymethods]
impl HttpSpanExporter {
    #[new]
    #[pyo3(signature = (batch_export=true, export_config=None, http_config=None, sample_ratio=None))]
    pub fn new(
        batch_export: bool,
        export_config: Option<&ExportConfig>,
        http_config: Option<&HttpConfig>,
        sample_ratio: Option<f64>,
    ) -> Result<Self, TraceError> {
        let (endpoint, protocol, timeout) = if let Some(config) = export_config {
            (
                config.endpoint.clone(),
                config.protocol.clone(),
                config.timeout,
            )
        } else {
            (None, Protocol::default(), None)
        };

        let headers = http_config.and_then(|cfg| cfg.headers.clone());
        let compression = if let Some(http_config) = http_config {
            if let Some(compression) = &http_config.compression {
                Some(compression.clone())
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            batch_export,
            sample_ratio,
            endpoint,
            protocol,
            timeout,
            headers,
            compression,
        })
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl SpanExporterBuilder for HttpSpanExporter {
    type Exporter = OtlpSpanExporter;

    fn sample_ratio(&self) -> Option<f64> {
        self.sample_ratio
    }

    fn batch_export(&self) -> bool {
        self.batch_export
    }

    fn build_exporter(&self) -> Result<Self::Exporter, TraceError> {
        // Reconstruct the OtlpExportConfig each time
        let timeout = self.timeout.map(Duration::from_secs);
        let export_config = OtlpExportConfig {
            endpoint: self.endpoint.clone(),
            protocol: self.protocol.to_otel_protocol(),
            timeout,
        };

        let mut exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_export_config(export_config);

        if let Some(headers) = &self.headers {
            exporter = exporter.with_headers(headers.clone());
        }

        if let Some(compression) = &self.compression {
            exporter = exporter.with_compression(compression.to_otel_compression()?);
        }

        Ok(exporter.build()?)
    }
}
