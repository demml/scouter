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
use scouter_state::app_state;
use scouter_types::{MessageRecord, TraceServerRecord};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, error, instrument};
pub struct ScouterSpanExporter {
    space: String,
    name: String,
    version: String,
    producer: Arc<RustScouterProducer>,
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
        let producer = app_state()
            .handle()
            .block_on(async { RustScouterProducer::new(transport_config).await })?;
        Ok(ScouterSpanExporter {
            space,
            name,
            version,
            producer: Arc::new(producer),
        })
    }
}

impl SpanExporter for ScouterSpanExporter {
    #[instrument(name = "ScouterSpanExporter::export", skip_all)]
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let producer = self.producer.clone(); // Requires RustScouterProducer: Clone
        let space = self.space.clone();
        let name = self.name.clone();
        let version = self.version.clone();

        debug!("Preparing to export {} spans to Scouter", batch.len());
        let export_future = async move {
            // Note: No explicit type annotation here
            let resource_spans =
                group_spans_by_resource_and_scope(batch, &ResourceAttributesWithSchema::default());
            let req = ExportTraceServiceRequest { resource_spans };

            // Note: `self` is consumed by the async move block.
            let record = TraceServerRecord {
                request: req,
                space,
                name,
                version,
            };
            let message = MessageRecord::TraceServerRecord(record);

            // This fallible call requires the block to resolve to a Result
            producer.publish(message).await.map_err(|e| {
                let msg = format!("Failed to publish message to scouter: {}", e);
                error!("{}", msg);
                OTelSdkError::InternalFailure(msg)
            })?;

            // Explicitly return the Ok(()) that the outer spawn expects
            Ok(()) as Result<(), OTelSdkError>
        };

        let runtime_handle = app_state().handle();
        runtime_handle
            .spawn(export_future)
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("Task spawn failed: {}", e)))?
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
