use scouter_sql::PostgresClient;



pub struct AppState {
    pub db: PostgresClient,
}
