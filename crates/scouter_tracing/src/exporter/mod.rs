pub mod grpc;
pub mod http;
pub mod noop;
pub mod processor;
pub mod scouter;
pub mod stdout;
pub mod testing;
pub mod traits;

use crate::error::TraceError;
use crate::exporter::noop::NoopSpanExporter;
use crate::exporter::processor::BatchConfig;
use crate::exporter::scouter::ScouterSpanExporter;
use crate::exporter::traits::SpanExporterBuilder;
use opentelemetry_sdk::Resource;
use pyo3::prelude::*;
use scouter_state::app_state;
use tracing::debug;

pub use grpc::GrpcSpanExporter;
pub use http::HttpSpanExporter;
pub use stdout::StdoutSpanExporter;
pub use testing::TestSpanExporter;

#[derive(PartialEq)]
pub enum ExporterType {
    Http,
    Stdout,
    Testing,
    Noop,
    Grpc,
}

// Enum for handling different span exporter types
#[derive(Debug)]
pub enum SpanExporterNum {
    Http(HttpSpanExporter),
    Stdout(StdoutSpanExporter),
    Testing(TestSpanExporter),
    Noop(NoopSpanExporter),
    Grpc(GrpcSpanExporter),
}

impl SpanExporterNum {
    pub fn from_pyobject(obj: &Bound<'_, PyAny>) -> Result<Self, TraceError> {
        if obj.is_instance_of::<HttpSpanExporter>() {
            let exporter = obj.extract::<HttpSpanExporter>()?;
            Ok(SpanExporterNum::Http(exporter))
        } else if obj.is_instance_of::<GrpcSpanExporter>() {
            let exporter = obj.extract::<GrpcSpanExporter>()?;
            Ok(SpanExporterNum::Grpc(exporter))
        } else if obj.is_instance_of::<StdoutSpanExporter>() {
            let exporter = obj.extract::<StdoutSpanExporter>()?;
            Ok(SpanExporterNum::Stdout(exporter))
        } else if obj.is_instance_of::<TestSpanExporter>() {
            let exporter = obj.extract::<TestSpanExporter>()?;
            Ok(SpanExporterNum::Testing(exporter))
        } else {
            debug!("Using NoopSpanExporter as default");
            Ok(SpanExporterNum::Noop(NoopSpanExporter::default()))
        }
    }

    pub(crate) fn set_sample_ratio(&mut self, sample_ratio: Option<f64>) {
        match self {
            SpanExporterNum::Http(builder) => builder.set_sample_ratio(sample_ratio),
            SpanExporterNum::Stdout(builder) => builder.set_sample_ratio(sample_ratio),
            SpanExporterNum::Testing(builder) => builder.set_sample_ratio(sample_ratio),
            SpanExporterNum::Noop(builder) => builder.set_sample_ratio(sample_ratio),
            SpanExporterNum::Grpc(builder) => builder.set_sample_ratio(sample_ratio),
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
            SpanExporterNum::Noop(builder) => {
                builder.build_provider(resource, scouter_exporter, batch_config)
            }
            // tonic requires a tokio runtime to start the background channel
            SpanExporterNum::Grpc(builder) => app_state().block_on(async {
                builder.build_provider(resource, scouter_exporter, batch_config)
            }),
        }
    }
}

impl Default for SpanExporterNum {
    fn default() -> Self {
        SpanExporterNum::Noop(NoopSpanExporter::default())
    }
}
