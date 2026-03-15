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
use scouter_types::{MessageRecord, TraceServerRecord, TraceSpanRecord};
use std::fmt;
use std::sync::{Arc, RwLock};
use tracing::{error, instrument};

pub struct ScouterSpanExporter {
    producer: Arc<RustScouterProducer>,
    resource: Resource,
    capture_buffer: Arc<RwLock<Option<Vec<TraceSpanRecord>>>>,
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
            capture_buffer: Arc::new(RwLock::new(None)),
        })
    }

    pub fn capture_buffer_arc(&self) -> Arc<RwLock<Option<Vec<TraceSpanRecord>>>> {
        self.capture_buffer.clone()
    }
}

impl SpanExporter for ScouterSpanExporter {
    #[instrument(name = "ScouterSpanExporter::export", skip_all)]
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let producer = self.producer.clone();
        let resource = self.resource.clone();
        let capture_buffer = self.capture_buffer.clone();

        let export_future = async move {
            let resource_spans = group_spans_by_resource_and_scope(
                batch,
                &ResourceAttributesWithSchema::from(&resource),
            );
            let req = ExportTraceServiceRequest { resource_spans };
            let record = TraceServerRecord { request: req };

            // Check capture mode BEFORE consuming record into MessageRecord
            let is_capturing = capture_buffer.read().unwrap().is_some();

            if is_capturing {
                let (spans, _, _) = record
                    .to_records()
                    .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
                if let Some(buf) = capture_buffer.write().unwrap().as_mut() {
                    buf.extend(spans);
                }
                return Ok(());
            }

            let message = MessageRecord::TraceServerRecord(record);

            producer.publish(message).await.map_err(|e| {
                let msg = format!("Failed to publish message to scouter: {}", e);
                error!("{}", msg);
                OTelSdkError::InternalFailure(msg)
            })?;

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
