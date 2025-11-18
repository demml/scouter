use crate::exporter::ExporterType;
use crate::exporter::SpanExporterBuilder;
use crate::exporter::TraceError;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
#[derive(Debug, Default)]
pub struct NoopSpanExporter {
    _private: (),
}

impl NoopSpanExporter {
    /// Create a new noop span exporter
    pub fn new() -> Self {
        NoopSpanExporter { _private: () }
    }
}

impl SpanExporter for NoopSpanExporter {
    async fn export(&self, _: Vec<SpanData>) -> OTelSdkResult {
        Ok(())
    }
}

impl SpanExporterBuilder for NoopSpanExporter {
    type Exporter = OTelStdoutSpanExporter;

    fn export_type(&self) -> ExporterType {
        ExporterType::Noop
    }

    fn sample_ratio(&self) -> Option<f64> {
        None
    }

    fn batch_export(&self) -> bool {
        true
    }

    fn build_exporter(&self) -> Result<Self::Exporter, TraceError> {
        // Reconstruct the OtlpExportConfig each time
        Ok(OTelStdoutSpanExporter::default())
    }
}
