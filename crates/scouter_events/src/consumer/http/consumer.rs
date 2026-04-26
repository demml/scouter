use crate::consumer::utils::{process_server_records, process_tag_record, process_trace_record};
use flume::{Receiver, Sender};
use scouter_types::{ServerRecords, TagRecord, TraceServerRecord};
use sqlx::Pool;
use sqlx::Postgres;
use tokio::sync::watch;
use tracing::info;

pub struct MessageConsumerManager {
    pub server_record_tx: Sender<ServerRecords>,
    pub trace_record_tx: Sender<TraceServerRecord>,
    pub tag_record_tx: Sender<TagRecord>,
}

impl MessageConsumerManager {
    pub async fn start_server_record_worker(
        id: usize,
        consumer: Receiver<ServerRecords>,
        db_pool: Pool<Postgres>,
        mut shutdown: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    while let Ok(records) = consumer.try_recv() {
                        process_server_records(id, records, &db_pool).await;
                    }
                    info!("Server record consumer {}: Shutting down", id);
                    break;
                }
                result = consumer.recv_async() => {
                    match result {
                        Ok(records) => { process_server_records(id, records, &db_pool).await; }
                        Err(e) => {
                            info!("Server record consumer {}: Channel closed: {}", id, e);
                            break;
                        }
                    }
                }
            }
        }
    }

    pub async fn start_trace_worker(
        id: usize,
        consumer: Receiver<TraceServerRecord>,
        db_pool: Pool<Postgres>,
        mut shutdown: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    while let Ok(record) = consumer.try_recv() {
                        process_trace_record(id, record, &db_pool).await;
                    }
                    info!("Trace consumer {}: Shutting down", id);
                    break;
                }
                result = consumer.recv_async() => {
                    match result {
                        Ok(record) => { process_trace_record(id, record, &db_pool).await; }
                        Err(e) => {
                            info!("Trace consumer {}: Channel closed: {}", id, e);
                            break;
                        }
                    }
                }
            }
        }
    }

    pub async fn start_tag_worker(
        id: usize,
        consumer: Receiver<TagRecord>,
        db_pool: Pool<Postgres>,
        mut shutdown: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    while let Ok(record) = consumer.try_recv() {
                        process_tag_record(id, record, &db_pool).await;
                    }
                    info!("Tag consumer {}: Shutting down", id);
                    break;
                }
                result = consumer.recv_async() => {
                    match result {
                        Ok(record) => { process_tag_record(id, record, &db_pool).await; }
                        Err(e) => {
                            info!("Tag consumer {}: Channel closed: {}", id, e);
                            break;
                        }
                    }
                }
            }
        }
    }
}
