\
use opentelemetry_sdk::{trace::{SpanExporter, SpanData}, error::OTelSdkResult};


use crate::error::TraceError;

#[derive(Debug)]
pub struct ScouterSpanExporter {}

impl SpanExporter for ScouterSpanExporter {
    async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
        // Here you would implement the logic to export spans to Scouter
        for span in batch {
            // Convert SpanData to the format required by Scouter and send it
        }
        Ok(())
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        // Clean up resources if necessary
        Ok(())
    }
}
