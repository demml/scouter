use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};

use crate::error::TraceError;

#[derive(Debug)]
pub struct ScouterSpanExporter {}

impl SpanExporter for ScouterSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        // Here you would implement the logic to export spans to Scouter
        let resource_spans =
            group_spans_by_resource_and_scope(batch, &ResourceAttributesWithSchema::default());
        let req = ExportTraceServiceRequest { resource_spans };
        serde_json::to_string_pretty(&req)
        Ok(())`
    }`

    fn shutdown(&mut self) -> OTelSdkResult {
        // Clean up resources if necessary
        Ok(())
    }
}
