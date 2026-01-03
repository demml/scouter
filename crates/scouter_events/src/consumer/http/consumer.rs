use flume::{Receiver, Sender};
use metrics::counter;
use scouter_sql::MessageHandler;
use scouter_types::MessageRecord;
use sqlx::Pool;
use sqlx::Postgres;
use std::result::Result::Ok;
use tokio::sync::watch;
use tracing::{error, info};

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
                            let result = match records {

                                MessageRecord::ServerRecords(records) => {
                                    MessageHandler::insert_server_records(&db_pool, records).await
                                }
                                MessageRecord::TraceServerRecord(trace_record) => {
                                    MessageHandler::insert_trace_server_record(&db_pool, trace_record).await

                                }
                                MessageRecord::TagServerRecord(tag_record) => {
                                    MessageHandler::insert_tag_record(&db_pool, tag_record).await
                                }
                            };

                            if let Err(e) = result {
                                error!("Http consumer {}: Error handling message: {}", id, e);
                                counter!("db_insert_errors_from_http_consumer").increment(1);
                            } else {
                                match &records {
                                    MessageRecord::ServerRecords(server_records) => {
                                        counter!("records_inserted_from_http_consumer").absolute(server_records.len() as u64);
                                    }
                                    MessageRecord::TraceServerRecord(_) | MessageRecord::TagServerRecord(_) => {
                                        counter!("records_inserted_from_http_consumer").absolute(1);
                                    }

                                }
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
