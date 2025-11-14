use crate::error::TraceError;
use crate::exporter::processor::BatchConfig;
use crate::exporter::processor::EnrichSpanWithBaggageProcessor;
use crate::exporter::scouter::ScouterSpanExporter;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::SpanExporter;
use opentelemetry_sdk::Resource;
use tracing::info;
/// Common interface for all span exporter builders
pub trait SpanExporterBuilder {
    type Exporter: SpanExporter + 'static;

    /// Get the sampling ratio for this exporter
    fn sample_ratio(&self) -> Option<f64>;

    /// Whether to use simple or batch exporter
    fn batch_export(&self) -> bool;

    /// Build the actual span exporter - this is non-consuming
    fn build_exporter(&self) -> Result<Self::Exporter, TraceError>;

    /// Convert sample ratio to OpenTelemetry sampler
    fn to_sampler(&self) -> Sampler {
        self.sample_ratio()
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn)
    }

    /// Build a complete tracer provider with both this exporter and a Scouter exporter
    /// # Arguments
    /// * `resource` - The resource to associate with the tracer provider
    /// * `scouter_exporter` - The Scouter span exporter to include
    /// # Returns
    /// A fully built `SdkTracerProvider`
    fn build_provider(
        &self,
        resource: Resource,
        scouter_exporter: ScouterSpanExporter,
        batch_config: Option<BatchConfig>,
    ) -> Result<opentelemetry_sdk::trace::SdkTracerProvider, TraceError>
    where
        Self: Sized,
    {
        let exporter = self.build_exporter()?;
        let use_batch = self.batch_export();
        let sampler = self.to_sampler();

        let mut builder = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_span_processor(EnrichSpanWithBaggageProcessor);

        if use_batch {
            info!("Using batch span processor for exporter");
            let config = batch_config.unwrap_or_default();

            let exporter_batch_processor = BatchSpanProcessor::builder(exporter)
                .with_batch_config(config.to_otlp_config())
                .build();
            let scouter_batch_processor = BatchSpanProcessor::builder(scouter_exporter)
                .with_batch_config(config.to_otlp_config())
                .build();
            builder = builder
                .with_span_processor(exporter_batch_processor)
                .with_span_processor(scouter_batch_processor);
            //.with_batch_exporter(exporter)
            //.with_batch_exporter(scouter_exporter);
        } else {
            builder = builder
                .with_simple_exporter(exporter)
                .with_simple_exporter(scouter_exporter);
        }

        builder = builder.with_sampler(sampler).with_resource(resource);

        Ok(builder.build())
    }
}
