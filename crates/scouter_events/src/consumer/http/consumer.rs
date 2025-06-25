use flume::{Receiver, Sender};
use metrics::counter;
use scouter_sql::MessageHandler;
use scouter_types::ServerRecords;
use sqlx::Pool;
use sqlx::Postgres;
use std::result::Result::Ok;
use tokio::sync::watch;
use tracing::{error, info};

pub struct HttpConsumerManager {
    pub tx: Sender<ServerRecords>,
}

impl HttpConsumerManager {
    pub async fn start_worker(
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
                                error!("Http consumer {}: Error handling message: {}", id, e);
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
