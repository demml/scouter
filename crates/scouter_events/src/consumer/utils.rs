use metrics::counter;
use scouter_sql::MessageHandler;
use scouter_types::{ServerRecords, TagRecord, TraceServerRecord};
use sqlx::{Pool, Postgres};
use tracing::{error, instrument};

#[instrument(skip_all)]
pub(crate) async fn process_server_records(
    id: usize,
    records: ServerRecords,
    db_pool: &Pool<Postgres>,
) -> bool {
    let count = records.len();
    match MessageHandler::insert_server_records(db_pool, records).await {
        Ok(_) => {
            counter!("records_inserted").increment(count as u64);
            counter!("messages_processed").increment(1);
            true
        }
        Err(e) => {
            error!("Worker {}: Failed to insert server records: {:?}", id, e);
            counter!("db_insert_errors").increment(1);
            false
        }
    }
}

#[instrument(skip_all)]
pub(crate) async fn process_trace_record(
    id: usize,
    record: TraceServerRecord,
    db_pool: &Pool<Postgres>,
) -> bool {
    match MessageHandler::insert_trace_server_record(db_pool, record).await {
        Ok(_) => {
            counter!("records_inserted").increment(1);
            counter!("messages_processed").increment(1);
            true
        }
        Err(e) => {
            error!("Worker {}: Failed to insert trace record: {:?}", id, e);
            counter!("db_insert_errors").increment(1);
            false
        }
    }
}

#[instrument(skip_all)]
pub(crate) async fn process_tag_record(
    id: usize,
    record: TagRecord,
    db_pool: &Pool<Postgres>,
) -> bool {
    match MessageHandler::insert_tag_record(db_pool, record).await {
        Ok(_) => {
            counter!("records_inserted").increment(1);
            counter!("messages_processed").increment(1);
            true
        }
        Err(e) => {
            error!("Worker {}: Failed to insert tag record: {:?}", id, e);
            counter!("db_insert_errors").increment(1);
            false
        }
    }
}
