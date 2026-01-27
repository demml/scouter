use crate::exporter::TraceError;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{SpanData, SpanExporter},
};
use scouter_events::producer::RustScouterProducer;
use scouter_events::queue::types::TransportConfig;
use scouter_state::app_state;
use scouter_types::{MessageRecord, TraceServerRecord};
use std::fmt;
use std::sync::Arc;
use tracing::{error, instrument};
pub struct ScouterSpanExporter {
    producer: Arc<RustScouterProducer>,
    resource: Resource,
}

impl fmt::Debug for ScouterSpanExporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScouterSpanExporter").finish()
    }
}

impl ScouterSpanExporter {
    pub fn new(transport_config: TransportConfig, resource: &Resource) -> Result<Self, TraceError> {
        let producer = app_state()
            .handle()
            .block_on(async { RustScouterProducer::new(transport_config).await })?;
        Ok(ScouterSpanExporter {
            producer: Arc::new(producer),
            resource: resource.clone(),
        })
    }
}

impl SpanExporter for ScouterSpanExporter {
    #[instrument(name = "ScouterSpanExporter::export", skip_all)]
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let resource_spans = group_spans_by_resource_and_scope(
            batch,
            &ResourceAttributesWithSchema::from(&self.resource),
        );

        let req = ExportTraceServiceRequest { resource_spans };
        let record = TraceServerRecord { request: req };
        let message = MessageRecord::TraceServerRecord(record);

        self.producer.publish(message).await.map_err(|e| {
            let msg = format!("Failed to publish message to scouter: {}", e);
            error!("{}", msg);
            OTelSdkError::InternalFailure(msg)
        })
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
