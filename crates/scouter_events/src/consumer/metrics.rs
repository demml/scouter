use metrics::{counter, Counter};

pub struct ConsumerMetrics {
    pub messages_processed: Counter,
    pub messages_too_large: Counter,
    pub consumer_errors: Counter,
    pub db_insert_errors: Counter,
    pub records_inserted: Counter,
}

impl ConsumerMetrics {
    pub fn new() -> Self {
        Self {
            messages_processed: counter!("messages_processed", "type" => "processed"),
            messages_too_large: counter!("messages_too_large", "type" => "error"),
            consumer_errors: counter!("consumer_errors", "type" => "error"),
            db_insert_errors: counter!("db_insert_errors", "type" => "error"),
            records_inserted: counter!("db_records_inserted", "type" => "processed"),
        }
    }
}