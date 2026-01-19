use metrics::counter;
use scouter_sql::MessageHandler;
use scouter_types::{MessageRecord, MessageType};
use sqlx::{Pool, Postgres};
use tracing::{error, instrument};

/// Generalized function to process a MessageRecord and insert it into the database
/// # Arguments
/// * `id` - The worker ID
/// * `records` - The MessageRecord to process
/// * `db_pool` - The database pool
/// # Returns
/// * `Result<(), FeatureQueueError>` - The result of the operation
#[instrument(skip_all)]
pub(crate) async fn process_message_record(
    id: usize,
    records: MessageRecord,
    db_pool: &Pool<Postgres>,
) -> bool {
    let message_type = &records.record_type();
    let message_count = &records.len();
    let result = match records {
        MessageRecord::ServerRecords(records) => {
            MessageHandler::insert_server_records(db_pool, records).await
        }
        MessageRecord::TraceServerRecord(trace_records) => {
            MessageHandler::insert_trace_server_record(db_pool, trace_records).await
        }
        MessageRecord::TagServerRecord(tag_records) => {
            MessageHandler::insert_tag_record(db_pool, tag_records).await
        }
    };

    if let Err(e) = result {
        error!(
            "Worker {}: Failed to insert record: {:?}, record type: {:?}",
            id, e, message_type
        );
        counter!("db_insert_errors").increment(1);
        return false;
    } else {
        match message_type {
            MessageType::Server => {
                counter!("records_inserted").increment(*message_count as u64);
            }
            MessageType::Trace | MessageType::Tag => {
                counter!("records_inserted").increment(1);
            }
        }
        counter!("messages_processed").increment(1);
        return true;
    }
}
