
use crate::sql::postgres::PostgresClient;

pub struct AppState {
    pub db: PostgresClient,
}