use crate::parquet::dataset::engine::DatasetTableCommand;
use arrow_array::RecordBatch;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Per-table buffer actor that accumulates `RecordBatch` objects and flushes
/// them to the engine actor on capacity or timer triggers.
///
/// Sends `Vec<RecordBatch>` directly to the engine — Delta Lake's `write()`
/// accepts multiple batches natively, avoiding `concat_batches` copies.
pub struct DatasetBufferActor;

impl DatasetBufferActor {
    pub fn start(
        engine_tx: mpsc::Sender<DatasetTableCommand>,
        mut batch_rx: mpsc::Receiver<RecordBatch>,
        mut shutdown_rx: mpsc::Receiver<()>,
        flush_interval_secs: u64,
        max_buffer_rows: usize,
        table_fqn: String,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer: Vec<RecordBatch> = Vec::new();
            let mut row_count: usize = 0;
            let mut flush_ticker = interval(Duration::from_secs(flush_interval_secs));
            flush_ticker.tick().await; // skip immediate

            loop {
                tokio::select! {
                    batch_opt = batch_rx.recv() => {
                        match batch_opt {
                            Some(batch) => {
                                row_count += batch.num_rows();
                                buffer.push(batch);
                                if row_count >= max_buffer_rows {
                                    Self::flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                                }
                            }
                            None => {
                                // Channel closed — flush and exit
                                if !buffer.is_empty() {
                                    Self::flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                                }
                                break;
                            }
                        }
                    }
                    _ = flush_ticker.tick() => {
                        if !buffer.is_empty() {
                            Self::flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        if !buffer.is_empty() {
                            Self::flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                        }
                        break;
                    }
                }
            }

            info!("Buffer actor shut down for [{}]", table_fqn);
        })
    }

    async fn flush(
        engine_tx: &mpsc::Sender<DatasetTableCommand>,
        buffer: &mut Vec<RecordBatch>,
        row_count: &mut usize,
        table_fqn: &str,
    ) {
        let batches = std::mem::take(buffer);
        let flushed_rows = *row_count;
        *row_count = 0;

        let (tx, rx) = oneshot::channel();
        if engine_tx
            .send(DatasetTableCommand::Write {
                batches,
                respond_to: tx,
            })
            .await
            .is_err()
        {
            error!("Engine channel closed for [{}]", table_fqn);
            return;
        }

        match rx.await {
            Ok(Ok(())) => {
                info!(
                    "Flushed {} rows to engine [{}]",
                    flushed_rows, table_fqn
                );
            }
            Ok(Err(e)) => {
                error!("Write failed for [{}]: {}", table_fqn, e);
            }
            Err(_) => {
                error!("Engine dropped response channel for [{}]", table_fqn);
            }
        }
    }
}
