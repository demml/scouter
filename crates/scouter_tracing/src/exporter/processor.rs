use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::Span;
use opentelemetry::Context;
use opentelemetry::KeyValue;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SpanProcessor;

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
