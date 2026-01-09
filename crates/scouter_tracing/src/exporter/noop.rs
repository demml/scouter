use crate::exporter::ExporterType;
use crate::exporter::SpanExporterBuilder;
use crate::exporter::TraceError;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanData;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::Resource;
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
#[derive(Debug, Default)]
pub struct NoopSpanExporter {
    pub sample_ratio: Option<f64>,
}

impl NoopSpanExporter {
    pub fn new() -> Self {
        NoopSpanExporter { sample_ratio: None }
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
        self.sample_ratio
    }

    fn batch_export(&self) -> bool {
        true
    }

    fn set_sample_ratio(&mut self, sample_ratio: Option<f64>) {
        self.sample_ratio = sample_ratio;
    }

    fn build_exporter(&self, resource: &Resource) -> Result<Self::Exporter, TraceError> {
        // Reconstruct the OtlpExportConfig each time
        let mut exporter = OTelStdoutSpanExporter::default();
        exporter.set_resource(resource);
        Ok(exporter)
    }
}
