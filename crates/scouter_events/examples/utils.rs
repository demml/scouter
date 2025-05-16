use scouter_error::LoggingError;
use scouter_settings::ScouterServerConfig;
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use std::io;
use tracing_subscriber::fmt::time::UtcTime;

const DEFAULT_TIME_PATTERN: &str =
    "[year]-[month]-[day]T[hour repr:24]:[minute]:[second]::[subsecond digits:4]";

// TODO: add ability to configure log level

/// Setup logging for the application
///
/// This function initializes the logging system for the application
pub async fn setup_logging() -> Result<(), LoggingError> {
    let time_format = time::format_description::parse(DEFAULT_TIME_PATTERN).unwrap();

    tracing_subscriber::fmt()
        .json()
        .with_ansi(true)
        .with_target(false)
        .flatten_event(true)
        .with_thread_ids(true)
        .with_timer(UtcTime::new(time_format))
        .with_writer(io::stdout)
        .try_init()
        .map_err(|e| LoggingError::Error(e.to_string()))?;

    Ok(())
}

pub async fn cleanup(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(
        r#"
        DELETE 
        FROM scouter.spc_drift;

        DELETE 
        FROM scouter.observability_metric;

        DELETE
        FROM scouter.custom_metric;

        DELETE
        FROM scouter.drift_alert;

        DELETE
        FROM scouter.drift_profile;

        DELETE
        FROM scouter.observed_bin_count;
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap();

    Ok(())
}

pub struct TestHelper {
    pub config: ScouterServerConfig,
    pub db_pool: Pool<Postgres>,
}

impl TestHelper {
    pub async fn new() -> Self {
        std::env::set_var("KAFKA_BROKERS", "localhost:9092");
        std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
        std::env::set_var("REDIS_ADDR", "redis://127.0.0.1:6379");

        let config = ScouterServerConfig::default();

        let db_pool = PostgresClient::create_db_pool(&config.database_settings)
            .await
            .unwrap();

        cleanup(&db_pool).await.unwrap();

        Self { config, db_pool }
    }
}

pub trait Config {
    fn get_config(&self) -> ScouterServerConfig;
}

impl Config for TestHelper {
    fn get_config(&self) -> ScouterServerConfig {
        self.config.clone()
    }
}

#[allow(dead_code)]
fn main() {
    println!("This is not an example");
}
