use crate::exporter::TraceError;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{SpanData, SpanExporter},
};
use scouter_events::producer::RustScouterProducer;
use scouter_events::queue::types::TransportConfig;
use scouter_types::{MessageRecord, TraceServerRecord};
use std::fmt;

pub struct ScouterSpanExporter {
    space: String,
    name: String,
    version: String,
    producer: RustScouterProducer,
}

impl fmt::Debug for ScouterSpanExporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScouterSpanExporter")
            .field("space", &self.space)
            .field("name", &self.name)
            .field("version", &self.version)
            .finish()
    }
}

impl ScouterSpanExporter {
    pub fn new(
        space: String,
        name: String,
        version: String,
        transport_config: TransportConfig,
    ) -> Result<Self, TraceError> {
        let producer = scouter_state::block_on_safe(async {
            RustScouterProducer::new(transport_config).await
        })?;
        Ok(ScouterSpanExporter {
            space,
            name,
            version,
            producer,
        })
    }
}

impl SpanExporter for ScouterSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let resource_spans =
            group_spans_by_resource_and_scope(batch, &ResourceAttributesWithSchema::default());
        let req = ExportTraceServiceRequest { resource_spans };
        let record = TraceServerRecord {
            request: req,
            space: self.space.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        };
        let message = MessageRecord::TraceServerRecord(record);
        self.producer.publish(message).await.map_err(|e| {
            OTelSdkError::InternalFailure(format!(
                "Failed to publish message to scouter: {}",
                e.to_string()
            ))
        })?;

        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        // Clean up resources if necessary
        Ok(())
    }
}
