use crate::error::TraceError;
use crate::exporter::traits::SpanExporterBuilder;
use crate::exporter::ExporterType;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::Resource;
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
use pyo3::prelude::*;
use scouter_types::PyHelperFuncs;
use serde::Serialize;
#[derive(Debug, Clone, Serialize, Default)]
#[pyclass]
pub struct StdoutSpanExporter {
    #[pyo3(get)]
    pub sample_ratio: Option<f64>,
    #[pyo3(get)]
    pub batch_export: bool,
}

#[pymethods]
impl StdoutSpanExporter {
    #[new]
    #[pyo3(signature = (batch_export=false, sample_ratio=None))]
    pub fn new(batch_export: bool, sample_ratio: Option<f64>) -> Self {
        StdoutSpanExporter {
            batch_export,
            sample_ratio,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl SpanExporterBuilder for StdoutSpanExporter {
    type Exporter = OTelStdoutSpanExporter;

    fn export_type(&self) -> ExporterType {
        ExporterType::Stdout
    }

    fn sample_ratio(&self) -> Option<f64> {
        self.sample_ratio
    }

    fn batch_export(&self) -> bool {
        self.batch_export
    }

    fn build_exporter(&self, resource: &Resource) -> Result<Self::Exporter, TraceError> {
        // Reconstruct the OtlpExportConfig each time
        let mut exporter = OTelStdoutSpanExporter::default();
        exporter.set_resource(resource);
        Ok(exporter)
    }
}
