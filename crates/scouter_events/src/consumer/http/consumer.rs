use crate::consumer::utils::process_message_record;
use flume::{Receiver, Sender};
use scouter_types::MessageRecord;
use sqlx::Pool;
use sqlx::Postgres;
use std::result::Result::Ok;
use tokio::sync::watch;
use tracing::info;
pub struct HttpConsumerManager {
    pub tx: Sender<MessageRecord>,
}

impl HttpConsumerManager {
    pub async fn start_worker(
        id: usize,
        consumer: Receiver<MessageRecord>,
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
                            process_message_record(id, records, &db_pool).await;
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
