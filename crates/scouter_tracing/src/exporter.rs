use std::collections::HashMap;

use crate::error::TraceError;
use crate::utils::Protocol;
use opentelemetry_otlp::ExportConfig as OtlpExportConfig;
use opentelemetry_otlp::SpanExporter as OtlpSpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
use pyo3::prelude::*;
use scouter_types::{records::TraceServerRecord, CompressionType};
use std::time::Duration;

#[derive(Debug)]
pub struct ScouterSpanExporter {
    pub space: String,
    pub name: String,
    pub version: String,
}

impl SpanExporter for ScouterSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        // Here you would implement the logic to export spans to Scouter
        let resource_spans =
            group_spans_by_resource_and_scope(batch, &ResourceAttributesWithSchema::default());
        let req = ExportTraceServiceRequest { resource_spans };
        let record = TraceServerRecord {
            request: req,
            space: self.space.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        };
        //let message_record = MessageRecord::TraceServerRecord(record);

        let (_traces, _span, _baggage) = record.to_records();

        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        // Clean up resources if necessary
        Ok(())
    }
}

#[derive(Debug)]
#[pyclass]
pub struct ExportConfig {
    #[pyo3(get)]
    pub endpoint: Option<String>,
    #[pyo3(get)]
    pub protocol: Protocol,
    #[pyo3(get)]
    pub timeout: Option<u64>,
}

#[pymethods]
impl ExportConfig {
    #[new]
    pub fn new(endpoint: Option<String>, protocol: Protocol, timeout: Option<u64>) -> Self {
        ExportConfig {
            endpoint,
            protocol,
            timeout,
        }
    }
}

impl ExportConfig {
    pub fn to_otel_config(&self) -> OtlpExportConfig {
        let duration: Option<Duration> = self.timeout.map(Duration::from_secs);
        let confing = OtlpExportConfig {
            endpoint: self.endpoint.clone(),
            protocol: self.protocol.to_otel_protocol(),
            timeout: duration,
        };
        confing
    }
}

#[derive(Debug)]
#[pyclass]
pub struct HttpConfig {
    headers: Option<HashMap<String, String>>,
    compression: Option<CompressionType>,
}

#[pymethods]
impl HttpConfig {
    #[new]
    pub fn new(
        headers: Option<HashMap<String, String>>,
        compression: Option<CompressionType>,
    ) -> Self {
        HttpConfig {
            headers,
            compression,
        }
    }
}

#[derive(Debug)]
#[pyclass]
pub struct HttpSpanExporter {
    pub exporter: OtlpSpanExporter,

    #[pyo3(get)]
    pub sample_ratio: Option<f64>,
}

#[pymethods]
impl HttpSpanExporter {
    #[new]
    #[pyo3(signature = (export_config=None, http_config=None, sample_ratio=None))]
    pub fn new(
        export_config: Option<&ExportConfig>,
        http_config: Option<&HttpConfig>,
        sample_ratio: Option<f64>,
    ) -> Result<Self, TraceError> {
        let export_config = export_config
            .map(|cfg| cfg.to_otel_config())
            .unwrap_or_default();

        let mut exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_export_config(export_config);

        if let Some(http_config) = http_config {
            if let Some(headers) = &http_config.headers {
                exporter = exporter.with_headers(headers.clone());
            }

            if let Some(compression) = &http_config.compression {
                let compression = compression.to_otel_compression()?;
                exporter = exporter.with_compression(compression);
            }
        }

        Ok(Self {
            exporter: exporter.build()?,
            sample_ratio,
        })
    }
}

impl HttpSpanExporter {
    pub fn sampler(&self) -> Sampler {
        self.sample_ratio
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn)
    }
}

#[derive(Debug)]
#[pyclass]
pub struct StdoutSpanExporter {
    pub exporter: OTelStdoutSpanExporter,
    sample_ratio: Option<f64>,
}

#[pymethods]
impl StdoutSpanExporter {
    #[new]
    pub fn new(sample_ratio: Option<f64>) -> Self {
        let exporter = OTelStdoutSpanExporter::default();
        StdoutSpanExporter {
            exporter,
            sample_ratio,
        }
    }
}

impl StdoutSpanExporter {
    pub fn sampler(&self) -> Sampler {
        self.sample_ratio
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn)
    }
}
