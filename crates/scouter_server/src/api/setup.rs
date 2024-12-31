use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::io;
use tracing::{error, info};

use tracing_subscriber;
use tracing_subscriber::fmt::time::UtcTime;

const DEFAULT_TIME_PATTERN: &str =
    "[year]-[month]-[day]T[hour repr:24]:[minute]:[second]::[subsecond digits:4]";

pub async fn setup_logging() -> Result<(), anyhow::Error> {
    let time_format = time::format_description::parse(DEFAULT_TIME_PATTERN).unwrap();

    tracing_subscriber::fmt()
        .json()
        .with_ansi(true)
        .with_target(false)
        .flatten_event(true)
        .with_thread_ids(true)
        .with_timer(UtcTime::new(time_format))
        .with_writer(io::stdout)
        .init();

    Ok(())
}

/// Setup the application with the given database pool.
pub async fn create_db_pool(database_url: Option<String>) -> Result<Pool<Postgres>, anyhow::Error> {
    // get env var
    let database_url = database_url.unwrap_or_else(|| {
        std::env::var("DATABASE_URL")
            .unwrap_or("postgresql://postgres:admin@localhost:5432/scouter?".to_string())
    });

    // get max connections from env or set to 10
    let max_connections = std::env::var("MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u32>()
        .expect("MAX_CONNECTIONS must be a number");

    let pool = match PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!("✅ Successfully connected to database");
            pool
        }
        Err(err) => {
            error!("🚨 Failed to connect to database {:?}", err);
            std::process::exit(1);
        }
    };

    Ok(pool)
}
