use crate::exporter::SpanExporterBuilder;
use crate::exporter::TraceError;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
/// Implementation for testing exporter used in unit testsuse opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};
use pyo3::prelude::*;
use scouter_types::error::RecordError;
use scouter_types::{records::TraceServerRecord, TraceBaggageRecord, TraceRecord, TraceSpanRecord};
use std::f32::consts::E;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
pub struct TestRecords {
    pub traces: Vec<TraceRecord>,
    pub spans: Vec<TraceSpanRecord>,
    pub baggage: Vec<TraceBaggageRecord>,
}

#[derive(Debug, Clone)]
#[pyclass]
pub struct TestSpanExporter {
    records: Arc<RwLock<TestRecords>>,
}

#[pymethods]
impl TestSpanExporter {
    #[new]
    pub fn new() -> Self {
        TestSpanExporter {
            records: Arc::new(RwLock::new(TestRecords {
                traces: Vec::new(),
                spans: Vec::new(),
                baggage: Vec::new(),
            })),
        }
    }

    #[getter]
    pub fn traces(&self) -> Vec<TraceRecord> {
        self.records.read().unwrap().traces.clone()
    }

    #[getter]
    pub fn spans(&self) -> Vec<TraceSpanRecord> {
        self.records.read().unwrap().spans.clone()
    }

    #[getter]
    pub fn baggage(&self) -> Vec<TraceBaggageRecord> {
        self.records.read().unwrap().baggage.clone()
    }
}

impl Default for TestSpanExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl SpanExporterBuilder for TestSpanExporter {
    type Exporter = OtelTestSpanExporter;

    fn sample_ratio(&self) -> Option<f64> {
        Some(1.0)
    }

    fn batch_export(&self) -> bool {
        false
    }

    fn build_exporter(&self) -> Result<Self::Exporter, TraceError> {
        Ok(OtelTestSpanExporter::new(self.records.clone()))
    }
}

#[derive(Debug)]
pub struct OtelTestSpanExporter {
    records: Arc<RwLock<TestRecords>>,
}

impl OtelTestSpanExporter {
    pub fn new(records: Arc<RwLock<TestRecords>>) -> Self {
        OtelTestSpanExporter { records }
    }
}

impl SpanExporter for OtelTestSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        // Here you would implement the logic to export spans to Scouter
        let resource_spans =
            group_spans_by_resource_and_scope(batch, &ResourceAttributesWithSchema::default());
        let req = ExportTraceServiceRequest { resource_spans };

        let record = TraceServerRecord {
            request: req,
            space: "test_space".to_string(),
            name: "test_name".to_string(),
            version: "test_version".to_string(),
        };

        let (traces, spans, baggage) = record
            .to_records()
            .map_err(|e| opentelemetry_sdk::error::OTelSdkError::InternalFailure(e.to_string()))?;

        let mut records = self.records.write().unwrap();
        records.traces.extend(traces);
        records.spans.extend(spans);
        records.baggage.extend(baggage);

        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
