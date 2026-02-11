use crate::error::TraceEngineError;
use crate::parquet::tracing::engine::{TableCommand, TraceSpanDBEngine};
use crate::parquet::tracing::queries::TraceQueries;
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
    pub query_service: TraceQueries,
}

impl TraceSpanService {
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        compaction_interval_hours: u64,
        flush_interval_secs: Option<u64>,
    ) -> Result<Self, TraceEngineError> {
        let engine = TraceSpanDBEngine::new(storage_settings).await?;
        info!(
            "TraceSpanService initialized with storage URI: {}",
            storage_settings.storage_uri
        );

        let ctx = engine.ctx.clone();
        let (engine_tx, engine_handle) = engine.start_actor(compaction_interval_hours);
        let (span_tx, span_rx) = mpsc::channel::<Vec<TraceSpan>>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        let buffer_handle = Self::start_buffering_actor(
            engine_tx.clone(),
            span_rx,
            shutdown_rx,
            flush_interval_secs,
        );

        Ok(TraceSpanService {
            engine_tx,
            span_tx,
            shutdown_tx,
            engine_handle,
            buffer_handle,
            query_service: TraceQueries::new(ctx),
        })
    }

    fn start_buffering_actor(
        engine_tx: mpsc::Sender<TableCommand>,
        mut span_rx: mpsc::Receiver<Vec<TraceSpan>>,
        mut shutdown_rx: mpsc::Receiver<()>,
        flush_interval_secs: Option<u64>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(BUFFER_SIZE);
            let mut flush_ticker = interval(Duration::from_secs(
                flush_interval_secs.unwrap_or(FLUSH_INTERVAL_SECS),
            ));
            flush_ticker.tick().await;

            loop {
                tokio::select! {
                    Some(spans) = span_rx.recv() => {

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
    use crate::parquet::tracing::span_view::TraceSpanView;
    use scouter_mocks::create_simple_trace;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::TraceId;
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
        let service = TraceSpanService::new(&storage_settings, 24, Some(2)).await?;
        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_dataframe_trace_write_single_batch() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2)).await?;

        let spans = create_simple_trace();
        info!("Test: writing {} spans", spans.len());
        service.write_spans(spans.clone()).await?;

        info!("Test: waiting for flush");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // get first span to verify to extract trace_id
        let first_span: &TraceSpan = spans.first().unwrap();
        // Convert hex string to binary bytes (16 bytes, not 32 bytes of UTF-8)
        let trace_id_bytes = TraceId::hex_to_bytes(&first_span.trace_id)?;

        info!("Test: querying spans for trace_id {:?}", trace_id_bytes);
        let records = service
            .query_service
            .get_trace_spans(Some(&trace_id_bytes), None, None, None, None)
            .await?;

        let total_spans: usize = records.iter().map(|batch| batch.len()).sum();
        println!(
            "Queried {} spans across {} batches",
            total_spans,
            records.len()
        );

        assert_eq!(
            total_spans, 3,
            "Expected to query 3 spans but got {}",
            total_spans
        );

        let span_views: Vec<TraceSpanView<'_>> = records
            .iter() // Iterator over &TraceSpanBatch
            .flat_map(|batch| batch.iter()) // batch.iter() creates TraceSpanView instances
            .collect();

        let serialized_spans = serde_json::to_string(&span_views).unwrap();

        // load back at vec<TraceSpan>
        let deserialized_spans: Vec<TraceSpan> = serde_json::from_str(&serialized_spans).unwrap();

        assert_eq!(
            deserialized_spans.len(),
            3,
            "Expected to deserialize 3 spans but got {}",
            deserialized_spans.len()
        );

        let last_span: &TraceSpan = spans.last().unwrap();

        let end_time = last_span.end_time;

        // query with time filter
        let records = service
            .query_service
            .get_trace_spans(None, None, None, Some(&end_time), None)
            .await?;

        // assert 3
        let total_spans: usize = records.iter().map(|batch| batch.len()).sum();
        assert_eq!(
            total_spans, 3,
            "Expected to query 3 spans with end_time filter but got {}",
            total_spans
        );

        info!("Test: shutting down");
        service.shutdown().await?;
        //cleanup();
        Ok(())
    }
}
