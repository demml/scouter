use crate::error::TraceError;
use crate::exporter::traits::SpanExporterBuilder;
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
use pyo3::prelude::*;
use scouter_types::PyHelperFuncs;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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

impl Default for StdoutSpanExporter {
    fn default() -> Self {
        Self {
            sample_ratio: None,
            batch_export: false,
        }
    }
}

impl SpanExporterBuilder for StdoutSpanExporter {
    type Exporter = OTelStdoutSpanExporter;

    fn sample_ratio(&self) -> Option<f64> {
        self.sample_ratio
    }

    fn batch_export(&self) -> bool {
        self.batch_export
    }

    fn build_exporter(&self) -> Result<Self::Exporter, TraceError> {
        Ok(OTelStdoutSpanExporter::default())
    }
}
