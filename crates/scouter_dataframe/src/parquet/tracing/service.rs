use crate::error::TraceEngineError;
use crate::parquet::tracing::engine::{TableCommand, TraceSpanDBEngine};
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::TraceSpan;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::info;
const BUFFER_SIZE: usize = 10_000;
const FLUSH_INTERVAL_SECS: u64 = 5;

pub struct TraceSpanService {
    engine_tx: mpsc::Sender<TableCommand>,
    span_tx: mpsc::Sender<Vec<TraceSpan>>,
    shutdown_tx: mpsc::Sender<()>,
    engine_handle: tokio::task::JoinHandle<()>,
    buffer_handle: tokio::task::JoinHandle<()>,
}

impl TraceSpanService {
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        compaction_interval_hours: u64,
    ) -> Result<Self, TraceEngineError> {
        let engine = TraceSpanDBEngine::new(storage_settings).await?;
        info!(
            "TraceSpanService initialized with storage URI: {}",
            storage_settings.storage_uri
        );

        let (engine_tx, engine_handle) = engine.start_actor(compaction_interval_hours);
        let (span_tx, span_rx) = mpsc::channel::<Vec<TraceSpan>>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        let buffer_handle = Self::start_buffering_actor(engine_tx.clone(), span_rx, shutdown_rx);

        Ok(TraceSpanService {
            engine_tx,
            span_tx,
            shutdown_tx,
            engine_handle,
            buffer_handle,
        })
    }

    fn start_buffering_actor(
        engine_tx: mpsc::Sender<TableCommand>,
        mut span_rx: mpsc::Receiver<Vec<TraceSpan>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(BUFFER_SIZE);
            let mut flush_ticker = interval(Duration::from_secs(FLUSH_INTERVAL_SECS));
            flush_ticker.tick().await;

            loop {
                tokio::select! {
                    Some(spans) = span_rx.recv() => {

                        println!("Buffering {} spans", spans.len());
                        buffer.extend(spans);

                        if buffer.len() >= BUFFER_SIZE {
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                    }
                    _ = flush_ticker.tick() => {
                        if !buffer.is_empty() {
                            println!("Flushing spans buffer with {} spans", buffer.len());
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Buffer actor received shutdown signal");
                        if !buffer.is_empty() {
                            info!("Flushing final {} spans before shutdown", buffer.len());
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                        break;
                    }
                }
            }

            info!("Buffering actor shutting down");
        })
    }

    async fn flush_buffer(engine_tx: &mpsc::Sender<TableCommand>, buffer: &mut Vec<TraceSpan>) {
        if buffer.is_empty() {
            return;
        }

        let spans_to_write = std::mem::replace(buffer, Vec::with_capacity(BUFFER_SIZE));
        let span_count = spans_to_write.len();

        info!("Sending write command to engine for {} spans", span_count);

        let (tx, rx) = tokio::sync::oneshot::channel();

        if let Err(e) = engine_tx
            .send(TableCommand::Write {
                spans: spans_to_write,
                respond_to: tx,
            })
            .await
        {
            tracing::error!("Failed to send write command: {}", e);
            return;
        }

        info!("Write command sent, waiting for response");

        match rx.await {
            Ok(Ok(())) => {
                info!("Successfully flushed {} spans", span_count);
            }
            Ok(Err(e)) => {
                tracing::error!("Write failed: {}", e);
            }
            Err(e) => {
                tracing::error!("Failed to receive write response: {}", e);
            }
        }
    }

    pub async fn write_spans(&self, spans: Vec<TraceSpan>) -> Result<(), TraceEngineError> {
        self.span_tx
            .send(spans)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        Ok(())
    }

    pub async fn optimize(&self) -> Result<(), TraceEngineError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.engine_tx
            .send(TableCommand::Optimize { respond_to: tx })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;

        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn shutdown(self) -> Result<(), TraceEngineError> {
        info!("TraceSpanService shutting down");

        let _ = self.shutdown_tx.send(()).await;

        if let Err(e) = self.buffer_handle.await {
            tracing::error!("Buffer handle error: {}", e);
        }

        self.engine_tx
            .send(TableCommand::Shutdown)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;

        if let Err(e) = self.engine_handle.await {
            tracing::error!("Engine handle error: {}", e);
        }

        info!("TraceSpanService shutdown complete");
        Ok(())
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    use crate::parquet::psi::dataframe_to_psi_drift_features;
    use crate::parquet::spc::dataframe_to_spc_drift_features;
    use crate::parquet::types::BinnedTableName;
    use crate::parquet::utils::BinnedMetricsExtractor;
    use chrono::Utc;
    use object_store::path::Path;
    use rand::Rng;
    use scouter_mocks::create_simple_trace;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::{
        BoxedGenAIEvalRecord, GenAIEvalRecord, PsiRecord, ServerRecord, ServerRecords, SpcRecord,
        Status,
    };
    use scouter_types::{CustomMetricRecord, GenAIEvalTaskResult, GenAIEvalWorkflowResult};
    use serde_json::Map;
    use serde_json::Value;
    use tracing_subscriber;

    fn cleanup() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_service_initialization() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24).await?;
        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_write_single_batch() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24).await?;

        let spans = create_simple_trace();
        info!("Test: writing {} spans", spans.len());
        service.write_spans(spans.clone()).await?;

        info!("Test: waiting for flush");
        tokio::time::sleep(Duration::from_secs(10)).await;

        info!("Test: shutting down");
        service.shutdown().await?;
        cleanup();
        Ok(())
    }
}
