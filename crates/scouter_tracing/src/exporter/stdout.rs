use crate::error::TraceError;
use crate::exporter::traits::SpanExporterBuilder;
use opentelemetry_stdout::SpanExporter as OTelStdoutSpanExporter;
use pyo3::prelude::*;

#[derive(Debug, Clone)]
#[pyclass]
pub struct StdoutSpanExporter {
    #[pyo3(get)]
    pub sample_ratio: Option<f64>,
    #[pyo3(get)]
    pub use_simple_exporter: bool,
}

#[pymethods]
impl StdoutSpanExporter {
    #[new]
    #[pyo3(signature = (use_simple_exporter=false, sample_ratio=None))]
    pub fn new(use_simple_exporter: bool, sample_ratio: Option<f64>) -> Self {
        StdoutSpanExporter {
            use_simple_exporter,
            sample_ratio,
        }
    }
}

impl Default for StdoutSpanExporter {
    fn default() -> Self {
        Self {
            sample_ratio: None,
            use_simple_exporter: false,
        }
    }
}

impl SpanExporterBuilder for StdoutSpanExporter {
    type Exporter = OTelStdoutSpanExporter;

    fn sample_ratio(&self) -> Option<f64> {
        self.sample_ratio
    }

    fn use_simple_exporter(&self) -> bool {
        self.use_simple_exporter
    }

    fn build_exporter(&self) -> Result<Self::Exporter, TraceError> {
        Ok(OTelStdoutSpanExporter::default())
    }
}
