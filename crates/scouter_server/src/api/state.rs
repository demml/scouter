use scouter_sql::PostgresClient;
use scouter_events::consumer::metrics::ConsumerMetrics;
use std::sync::Arc;



pub struct AppState {
    pub db: PostgresClient,
    pub metrics: Arc<ConsumerMetrics>,
}
