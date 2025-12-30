use crate::exporter::ExporterType;
use crate::exporter::SpanExporterBuilder;
use crate::exporter::TraceError;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::{error::OTelSdkResult, trace::SpanData};
use pyo3::prelude::*;
use scouter_types::TagRecord;
use scouter_types::{TraceBaggageRecord, TraceServerRecord, TraceSpanRecord};
use std::sync::{Arc, RwLock};
#[derive(Debug)]
pub struct TestRecords {
    pub tags: Vec<TagRecord>,
    pub spans: Vec<TraceSpanRecord>,
    pub baggage: Vec<TraceBaggageRecord>,
}

#[derive(Debug, Clone)]
#[pyclass]
pub struct TestSpanExporter {
    records: Arc<RwLock<TestRecords>>,
    batch_export: bool,
}

#[pymethods]
impl TestSpanExporter {
    #[new]
    #[pyo3(signature = (batch_export=true))]
    pub fn new(batch_export: bool) -> Self {
        TestSpanExporter {
            records: Arc::new(RwLock::new(TestRecords {
                tags: Vec::new(),
                spans: Vec::new(),
                baggage: Vec::new(),
            })),
            batch_export,
        }
    }

    #[getter]
    pub fn tags(&self) -> Vec<TagRecord> {
        self.records.read().unwrap().tags.clone()
    }

    #[getter]
    pub fn spans(&self) -> Vec<TraceSpanRecord> {
        self.records.read().unwrap().spans.clone()
    }

    #[getter]
    pub fn baggage(&self) -> Vec<TraceBaggageRecord> {
        self.records.read().unwrap().baggage.clone()
    }

    pub fn clear(&self) {
        let mut records = self.records.write().unwrap();
        records.tags.clear();
        records.spans.clear();
        records.baggage.clear();
    }
}

impl Default for TestSpanExporter {
    fn default() -> Self {
        Self::new(true)
    }
}

impl SpanExporterBuilder for TestSpanExporter {
    type Exporter = OtelTestSpanExporter;

    fn export_type(&self) -> ExporterType {
        ExporterType::Testing
    }

    fn sample_ratio(&self) -> Option<f64> {
        Some(1.0)
    }

    fn batch_export(&self) -> bool {
        self.batch_export
    }

    fn build_exporter(&self, resource: &Resource) -> Result<Self::Exporter, TraceError> {
        Ok(OtelTestSpanExporter::new(
            self.records.clone(),
            resource.clone(),
        ))
    }
}

#[derive(Debug)]
pub struct OtelTestSpanExporter {
    records: Arc<RwLock<TestRecords>>,
    resource: Resource,
}

impl OtelTestSpanExporter {
    pub fn new(records: Arc<RwLock<TestRecords>>, resource: Resource) -> Self {
        OtelTestSpanExporter { records, resource }
    }
}

impl SpanExporter for OtelTestSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        // Here you would implement the logic to export spans to Scouter
        let resource_spans = group_spans_by_resource_and_scope(
            batch,
            &ResourceAttributesWithSchema::from(&self.resource),
        );

        let req = ExportTraceServiceRequest { resource_spans };

        let record = TraceServerRecord { request: req };

        let (spans, baggage, tags) = record
            .to_records()
            .map_err(|e| opentelemetry_sdk::error::OTelSdkError::InternalFailure(e.to_string()))?;

        let mut records = self.records.write().unwrap();
        records.tags.extend(tags);
        records.spans.extend(spans);
        records.baggage.extend(baggage);

        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
