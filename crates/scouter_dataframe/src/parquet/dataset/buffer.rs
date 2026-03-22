use crate::parquet::dataset::engine::TableCommand;
use arrow_array::RecordBatch;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, Duration};
use tracing::{error, info};

/// Per-table buffer actor that accumulates `RecordBatch` objects and flushes
/// them to the engine actor on capacity or timer triggers.
///
/// Sends `Vec<RecordBatch>` directly to the engine — Delta Lake's `write()`
/// accepts multiple batches natively, avoiding `concat_batches` copies.
pub fn start_buffer(
    engine_tx: mpsc::Sender<TableCommand>,
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
                                flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                            }
                        }
                        None => {
                            // Channel closed — flush and exit
                            if !buffer.is_empty() {
                                flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                            }
                            break;
                        }
                    }
                }
                _ = flush_ticker.tick() => {
                    if !buffer.is_empty() {
                        flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                    }
                }
                _ = shutdown_rx.recv() => {
                    if !buffer.is_empty() {
                        flush(&engine_tx, &mut buffer, &mut row_count, &table_fqn).await;
                    }
                    break;
                }
            }
        }

        info!("Buffer actor shut down for [{}]", table_fqn);
    })
}

async fn flush(
    engine_tx: &mpsc::Sender<TableCommand>,
    buffer: &mut Vec<RecordBatch>,
    row_count: &mut usize,
    table_fqn: &str,
) {
    let batches = std::mem::take(buffer);
    let flushed_rows = *row_count;
    *row_count = 0;

    // Clone is O(n_columns) not O(n_rows) — columns are Arc<dyn Array>
    let batches_backup = batches.clone();

    let (tx, rx) = oneshot::channel();
    if engine_tx
        .send(TableCommand::Write {
            batches,
            respond_to: tx,
        })
        .await
        .is_err()
    {
        error!(
            "Engine channel closed for [{}] — restoring {} rows to buffer",
            table_fqn, flushed_rows
        );
        *buffer = batches_backup;
        *row_count = flushed_rows;
        return;
    }

    match rx.await {
        Ok(Ok(())) => {
            info!("Flushed {} rows to engine [{}]", flushed_rows, table_fqn);
        }
        Ok(Err(e)) => {
            error!(
                "Write failed for [{}]: {} — restoring {} rows to buffer",
                table_fqn, e, flushed_rows
            );
            *buffer = batches_backup;
            *row_count = flushed_rows;
        }
        Err(_) => {
            error!(
                "Engine dropped response channel for [{}] — restoring {} rows to buffer",
                table_fqn, flushed_rows
            );
            *buffer = batches_backup;
            *row_count = flushed_rows;
        }
    }
}
