pub mod http;
pub mod processor;
pub mod scouter;
pub mod stdout;
pub mod testing;
pub mod traits;

use crate::error::TraceError;
use crate::exporter::processor::BatchConfig;
use crate::exporter::scouter::ScouterSpanExporter;
use crate::exporter::traits::SpanExporterBuilder;
use opentelemetry_sdk::Resource;
use pyo3::prelude::*;

pub use http::HttpSpanExporter;
pub use stdout::StdoutSpanExporter;
pub use testing::TestSpanExporter;

// Enum for handling different span exporter types
#[derive(Debug)]
pub enum SpanExporterNum {
    Http(HttpSpanExporter),
    Stdout(StdoutSpanExporter),
    Testing(TestSpanExporter),
}

impl SpanExporterNum {
    pub fn from_pyobject(obj: &Bound<'_, PyAny>) -> Result<Self, TraceError> {
        if obj.is_instance_of::<HttpSpanExporter>() {
            let exporter = obj.extract::<HttpSpanExporter>()?;
            Ok(SpanExporterNum::Http(exporter))
        } else if obj.is_instance_of::<StdoutSpanExporter>() {
            let exporter = obj.extract::<StdoutSpanExporter>()?;
            Ok(SpanExporterNum::Stdout(exporter))
        } else if obj.is_instance_of::<TestSpanExporter>() {
            let exporter = obj.extract::<TestSpanExporter>()?;
            Ok(SpanExporterNum::Testing(exporter))
        } else {
            Err(TraceError::UnsupportedSpanExporterType)
        }
    }

    pub fn build_provider(
        &self,
        resource: Resource,
        scouter_exporter: ScouterSpanExporter,
        batch_config: Option<BatchConfig>,
    ) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, TraceError> {
        match self {
            SpanExporterNum::Http(builder) => {
                builder.build_provider(resource, scouter_exporter, batch_config)
            }
            SpanExporterNum::Stdout(builder) => {
                builder.build_provider(resource, scouter_exporter, batch_config)
            }
            SpanExporterNum::Testing(builder) => {
                builder.build_provider(resource, scouter_exporter, batch_config)
            }
        }
    }
}

impl Default for SpanExporterNum {
    fn default() -> Self {
        SpanExporterNum::Stdout(StdoutSpanExporter::default())
    }
}
