use crate::exporter::TraceError;
use crate::tracer::{CAPTURE_BUFFER, CAPTURE_BUFFER_MAX, CAPTURING};
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
use tracing::{error, instrument, warn};

/// Lazy-initialised span exporter.
///
/// The connection to the Scouter backend is established on the **first export**,
/// not at construction time. This ensures that `init_tracer()` (and therefore
/// any OTel auto-instrumentor that calls `get_tracer()`) never fails due to a
/// transient backend outage. If the backend is unavailable, spans are dropped
/// with a warning — consistent with the OTel exporter contract.
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

    /// Return an `Arc` to the producer, initialising it on first call.
    ///
    /// Returns a cloned `Arc` so the caller can move it into a spawned future
    /// without lifetime conflicts.  If two concurrent exports race to initialise,
    /// the loser's producer is silently discarded.
    async fn get_or_init_producer(&self) -> Result<Arc<RustScouterProducer>, TraceError> {
        if let Some(p) = self.producer.get() {
            return Ok(p.clone());
        }
        let producer = Arc::new(RustScouterProducer::new(self.transport_config.clone()).await?);
        let _ = self.producer.set(producer); // ignore Err — another task won the race
        Ok(self.producer.get().expect("producer was just set").clone())
    }
}

impl SpanExporter for ScouterSpanExporter {
    #[instrument(name = "ScouterSpanExporter::export", skip_all)]
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        let resource = self.resource.clone();

        // Fast path: local capture buffer (test / debug mode)
        if CAPTURING.load(Ordering::Acquire) {
            let resource_spans = group_spans_by_resource_and_scope(
                batch,
                &ResourceAttributesWithSchema::from(&resource),
            );
            let req = ExportTraceServiceRequest { resource_spans };
            let record = TraceServerRecord { request: req };
            let (spans, _, _) = record
                .to_records()
                .map_err(|e| OTelSdkError::InternalFailure(e.to_string()))?;
            let mut buf = CAPTURE_BUFFER.write().unwrap_or_else(|p| p.into_inner());
            let available = CAPTURE_BUFFER_MAX.saturating_sub(buf.len());
            if available == 0 {
                warn!(
                    "CAPTURE_BUFFER full ({} records); dropping new spans to prevent OOM",
                    CAPTURE_BUFFER_MAX
                );
            } else {
                if spans.len() > available {
                    warn!(
                        "CAPTURE_BUFFER near full; truncating batch from {} to {} spans",
                        spans.len(),
                        available
                    );
                }
                buf.extend(spans.into_iter().take(available));
            }
            return Ok(());
        }

        // Lazy-init the producer; drop spans with a warning if backend is unavailable.
        let producer = match self.get_or_init_producer().await {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    "ScouterSpanExporter: backend unavailable, dropping {} span(s): {}",
                    batch.len(),
                    e
                );
                return Ok(());
            }
        };

        let resource_spans = group_spans_by_resource_and_scope(
            batch,
            &ResourceAttributesWithSchema::from(&resource),
        );
        let req = ExportTraceServiceRequest { resource_spans };
        let record = TraceServerRecord { request: req };
        let message = MessageRecord::TraceServerRecord(record);

        // Move both `producer` (Arc clone) and `message` into the spawned future
        // to satisfy the `'static` bound required by `tokio::spawn`.
        let runtime_handle = app_state().handle();
        runtime_handle
            .spawn(async move {
                producer.publish(message).await.map_err(|e| {
                    let msg = format!("Failed to publish message to scouter: {}", e);
                    error!("{}", msg);
                    OTelSdkError::InternalFailure(msg)
                })
            })
            .await
            .map_err(|e| OTelSdkError::InternalFailure(format!("Task spawn failed: {}", e)))?
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        Ok(())
    }
}
