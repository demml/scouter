pub mod http;
pub mod processor;
pub mod scouter;
pub mod stdout;
pub mod traits;

use crate::error::TraceError;
use crate::exporter::scouter::ScouterSpanExporter;
use crate::exporter::traits::SpanExporterBuilder;
use opentelemetry_sdk::Resource;
use pyo3::prelude::*;
// Enum for handling different span exporter types
#[derive(Debug)]
pub enum SpanExporterNum {
    Http(http::HttpSpanExporter),
    Stdout(stdout::StdoutSpanExporter),
}

impl SpanExporterNum {
    pub fn from_pyobject(obj: &Bound<'_, PyAny>) -> Result<Self, TraceError> {
        if obj.is_instance_of::<http::HttpSpanExporter>() {
            let exporter = obj.extract::<http::HttpSpanExporter>()?;
            Ok(SpanExporterNum::Http(exporter))
        } else if obj.is_instance_of::<stdout::StdoutSpanExporter>() {
            let exporter = obj.extract::<stdout::StdoutSpanExporter>()?;
            Ok(SpanExporterNum::Stdout(exporter))
        } else {
            Err(TraceError::UnsupportedSpanExporterType)
        }
    }

    pub fn build_provider(
        &self,
        resource: Resource,
        scouter_exporter: ScouterSpanExporter,
    ) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, TraceError> {
        match self {
            SpanExporterNum::Http(builder) => builder.build_provider(resource, scouter_exporter),
            SpanExporterNum::Stdout(builder) => builder.build_provider(resource, scouter_exporter),
        }
    }
}

impl Default for SpanExporterNum {
    fn default() -> Self {
        SpanExporterNum::Stdout(stdout::StdoutSpanExporter::default())
    }
}
