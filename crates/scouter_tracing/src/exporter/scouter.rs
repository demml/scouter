use crate::exporter::TraceError;
use crate::tracer::CAPTURING;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::{
    error::{OTelSdkError, OTelSdkResult},
    trace::{SpanData, SpanExporter},
};
use std::sync::atomic::Ordering;

use scouter_events::producer::RustScouterProducer;
use scouter_events::queue::types::TransportConfig;
use scouter_state::app_state;
use scouter_types::{MessageRecord, TraceServerRecord};
use std::fmt;
use std::sync::{Arc, OnceLock};
use tracing::{debug, error, instrument, warn};

pub struct ScouterSpanExporter {
    transport_config: TransportConfig,
    resource: Resource,
    producer: Arc<OnceLock<Arc<RustScouterProducer>>>,
}

impl fmt::Debug for ScouterSpanExporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScouterSpanExporter").finish()
    }
}

impl ScouterSpanExporter {
    pub fn new(transport_config: TransportConfig, resource: &Resource) -> Result<Self, TraceError> {
        Ok(ScouterSpanExporter {
            transport_config,
            resource: resource.clone(),
            producer: Arc::new(OnceLock::new()),
        })
    }
}

impl SpanExporter for ScouterSpanExporter {
    #[instrument(name = "ScouterSpanExporter::export", skip_all)]
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let producer_lock = self.producer.clone();
        let transport_config = self.transport_config.clone();
        let resource = self.resource.clone();

        // Local capture buffer — dev/test only. While active, spans are routed to the
        // in-process buffer and NOT forwarded to the Scouter backend.
        if CAPTURING.load(Ordering::Acquire) {
            debug!(
                "ScouterSpanExporter: local capture active — spans buffered locally, not sent to Scouter backend"
            );
            let resource_spans = group_spans_by_resource_and_scope(
                batch,
                &ResourceAttributesWithSchema::from(&resource),
            );
            let req = ExportTraceServiceRequest { resource_spans };
            let record = TraceServerRecord { request: req };
            let (spans, _, _) = record
                .to_records()
                .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
            scouter_types::span_capture::buffer_captured_spans(spans);
            return Ok(());
        }

        let resource_spans = group_spans_by_resource_and_scope(
            batch,
            &ResourceAttributesWithSchema::from(&resource),
        );
        let message = MessageRecord::TraceServerRecord(TraceServerRecord {
            request: ExportTraceServiceRequest { resource_spans },
        });

        // Producer init (if needed) and publish both happen inside the spawned future
        // so they run on app_state()'s multi-threaded runtime — which has the reactor
        // required by hyper/tonic for DNS and TCP. Awaiting the JoinHandle propagates
        // publish errors back to the BatchSpanProcessor.
        app_state()
            .handle()
            .spawn(async move {
                let producer = match producer_lock.get() {
                    Some(p) => p.clone(),
                    None => match RustScouterProducer::new(transport_config).await {
                        Ok(p) => {
                            let _ = producer_lock.set(Arc::new(p));
                            producer_lock.get().expect("just set").clone()
                        }
                        Err(e) => {
                            warn!("ScouterSpanExporter: producer init failed, dropping spans: {e}");
                            return Ok(());
                        }
                    },
                };

                producer.publish(message).await.map_err(|e| {
                    let msg = format!("Failed to publish spans to scouter: {e}");
                    error!("{msg}");
                    OTelSdkError::InternalFailure(msg)
                })
            })
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("Task spawn failed: {e}")))?
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
