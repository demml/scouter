use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::Span;
use opentelemetry::Context;
use opentelemetry::KeyValue;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanProcessor;
use opentelemetry_sdk::trace::{BatchConfig as OTelBatchConfig, BatchConfigBuilder};
use pyo3::prelude::*;
use std::time::Duration;

// This is taken from the opentelemetry examples for adding baggage to spans
#[derive(Debug)]
pub(crate) struct EnrichSpanWithBaggageProcessor;

impl SpanProcessor for EnrichSpanWithBaggageProcessor {
    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }

    fn on_start(&self, span: &mut opentelemetry_sdk::trace::Span, cx: &Context) {
        for (kk, vv) in cx.baggage().iter() {
            span.set_attribute(KeyValue::new(kk.clone(), vv.0.clone()));
        }
    }

    fn on_end(&self, _span: opentelemetry_sdk::trace::SpanData) {}
}

#[pyclass]
#[derive(PartialEq, Clone, Debug)]
pub struct BatchConfig {
    pub max_queue_size: usize,
    pub scheduled_delay: Duration,
    pub max_export_batch_size: usize,
}

#[pymethods]
impl BatchConfig {
    #[new]
    #[pyo3(signature = (
        max_queue_size=2048,
        scheduled_delay_ms=5000,
        max_export_batch_size=512,
    ))]
    pub fn new(
        max_queue_size: usize,
        scheduled_delay_ms: u64,
        max_export_batch_size: usize,
    ) -> Self {
        BatchConfig {
            max_queue_size,
            scheduled_delay: Duration::from_millis(scheduled_delay_ms),
            max_export_batch_size,
        }
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        BatchConfig {
            max_queue_size: 2048,
            scheduled_delay: Duration::from_millis(5000),
            max_export_batch_size: 512,
        }
    }
}

impl BatchConfig {
    pub fn to_otlp_config(&self) -> OTelBatchConfig {
        let mut builder = BatchConfigBuilder::default();

        builder = builder
            .with_scheduled_delay(self.scheduled_delay)
            .with_max_queue_size(self.max_queue_size)
            .with_max_export_batch_size(self.max_export_batch_size);

        builder.build()
    }
}
