use crate::error::EventError;
use flume::{Receiver, Sender};
use metrics::counter;
use scouter_settings::events::HttpConsumerSettings;
use scouter_sql::MessageHandler;
use scouter_types::ServerRecords;
use sqlx::Pool;
use sqlx::Postgres;
use std::result::Result::Ok;
use tokio::sync::watch;
use tracing::log::debug;
use tracing::{error, info};

pub struct HttpConsumerManager {
    pub tx: Sender<ServerRecords>,
}

impl HttpConsumerManager {
    pub async fn new(
        consumer_settings: &HttpConsumerSettings,
        db_pool: &Pool<Postgres>,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<Self, EventError> {
        let (tx, rx) = flume::unbounded();
        let num_workers = consumer_settings.num_workers;

        Self::start_workers(num_workers, rx, shutdown_rx, db_pool).await;

        debug!("✅ Started {} HTTP consumers", num_workers);
        Ok(Self { tx })
    }

    async fn start_workers(
        num_workers: usize,
        rx: Receiver<ServerRecords>,
        shutdown_rx: watch::Receiver<()>,
        db_pool: &Pool<Postgres>,
    ) {
        for id in 0..num_workers {
            let consumer = rx.clone();
            let worker_shutdown_rx = shutdown_rx.clone();
            let db_pool_clone = db_pool.clone();

            tokio::spawn(async move {
                Self::start_worker(id, consumer, db_pool_clone, worker_shutdown_rx).await;
            });
        }
    }

    async fn start_worker(
        id: usize,
        consumer: Receiver<ServerRecords>,
        db_pool: Pool<Postgres>,
        mut shutdown: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Http consumer {}: Shutting down", id);
                    break;
                }
                result = consumer.recv_async() => {
                    match result {
                        Ok(records) => {
                            if let Err(e) = MessageHandler::insert_server_records(&db_pool,&records).await {
                                error!("Http consumer  {}: Error handling message: {}", id, e);
                                counter!("db_insert_errors_from_http_consumer").increment(1);
                            } else {
                                counter!("records_inserted_from_http_consumer").absolute(records.records.len() as u64);
                                counter!("messages_processed_from_http_consumer").increment(1);
                            }
                        }
                        Err(e) => {
                            info!("Http consumer Worker {} Channel closed: {}", id, e);
                            break;
                        }
                    }
                }
            }
        }
    }
}
