use std::collections::HashMap;

use opentelemetry_otlp::ExportConfig as OtlpExportConfig;
use opentelemetry_otlp::HasHttpConfig;
use opentelemetry_otlp::HttpExporterBuilderSet;
use opentelemetry_otlp::SpanExporterBuilder;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};
use pyo3::prelude::*;
use scouter_types::{records::TraceServerRecord, CompressionType};
use std::time::Duration;

use crate::utils::Protocol;

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
    exporter: SpanExporterBuilder<HttpExporterBuilderSet>,
}

#[pymethods]
impl HttpSpanExporter {
    #[new]
    #[pyo3(signature = (export_config=None))]
    pub fn new(export_config: Option<&ExportConfig>) -> Self {
        let export_config = match export_config {
            Some(cfg) => cfg.to_otel_config(),
            None => OtlpExportConfig::default(),
        };

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .http_client_config()
            .with_export_config(export_config);

        Self { exporter }
    }
}
