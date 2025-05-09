use crate::api::archive::DataArchiver;
use crate::api::drift_manager::BackgroundDriftManager;
use anyhow::{Context, Result as AnyhowResult};
use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger};
use scouter_settings::{
    DatabaseSettings, KafkaSettings, PollingSettings, RabbitMQSettings, ScouterServerConfig,
};
use scouter_sql::sql::schema::User;
use scouter_sql::sql::traits::UserSqlLogic;
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, info, instrument};

#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
use scouter_events::consumer::kafka::KafkaConsumerManager;

#[cfg(feature = "rabbitmq")]
use scouter_events::consumer::rabbitmq::RabbitMQConsumerManager;

// setup default users
/// This function is intended to be called the first time the server is started.
/// If there are no users in the database, it will create defaults.
/// It is recommended to change the password on first login for these default users.
/// The default users are:
/// * admin: admin/admin
/// * guest: guest/guest
pub async fn initialize_default_user(db_pool: &Pool<Postgres>) -> AnyhowResult<()> {
    // Check if any users exist
    let users = PostgresClient::get_users(db_pool)
        .await
        .context("❌ Failed to check existing users")?;

    // If users already exist, don't create a default user
    if !users.is_empty() {
        return Ok(());
    }

    // Create default admin user
    info!("Creating default admin user...");
    let default_username = std::env::var("SCOUTER_DEFAULT_USERNAME").unwrap_or("admin".to_string());
    let default_password = std::env::var("SCOUTER_DEFAULT_PASSWORD").unwrap_or("admin".to_string());
    let password_hash = password_auth::generate_hash(&default_password);

    // Create admin user with admin permissions
    let admin_user = User::new(
        default_username.clone(),
        password_hash,
        Some(vec!["read".to_string(), "write".to_string()]), // permissions
        Some(vec!["admin".to_string()]),                     // group_permissions
        Some("admin".to_string()),                           // role
    );

    // Insert the user
    PostgresClient::insert_user(db_pool, &admin_user)
        .await
        .context("❌ Failed to create default admin user")?;

    // create guest user
    let guest_user = User::new(
        "guest".to_string(),
        password_auth::generate_hash("guest"),
        Some(vec!["read".to_string(), "write:all".to_string()]),
        Some(vec!["user".to_string()]),
        Some("guest".to_string()),
    );

    // Insert the user
    PostgresClient::insert_user(db_pool, &guest_user)
        .await
        .context("❌ Failed to create default guest user")?;

    info!("✅ Created default admin and guest user (change password on first login)",);

    Ok(())
}

/// Setup logging for the application
pub async fn setup_logging() -> AnyhowResult<()> {
    let log_level = LogLevel::from_str(
        std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string())
            .as_str(),
    )?;

    let use_json = std::env::var("LOG_JSON")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()?;

    let config = LoggingConfig::new(Some(true), Some(log_level), None, Some(use_json));
    RustyLogger::setup_logging(Some(config))?;

    info!("Logging setup successfully");

    Ok(())
}

/// Get that database going!
pub async fn setup_database(db_settings: &DatabaseSettings) -> AnyhowResult<Pool<Postgres>> {
    let db_pool = PostgresClient::create_db_pool(db_settings)
        .await
        .with_context(|| "Failed to create Postgres client")?;

    initialize_default_user(&db_pool).await?;

    Ok(db_pool)
}

/// Helper to setup the kafka consumer
///
/// Arguments:
/// * `settings` - The kafka settings to use for the consumer
/// * `db_settings` - The database settings to use for the consumer
/// * `db_client` - The database client to use for the consumer
/// * `shutdown_rx` - The shutdown receiver to use for the consumer
///
/// Returns:
/// * `AnyhowResult<()>` - The result of the setup
pub async fn setup_kafka(
    settings: &KafkaSettings,
    db_pool: &Pool<Postgres>,
    shutdown_rx: tokio::sync::watch::Receiver<()>,
) -> AnyhowResult<()> {
    #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
    KafkaConsumerManager::start_workers(settings, db_pool, shutdown_rx).await?;
    info!("✅ Started Kafka workers");

    Ok(())
}

/// Helper to setup the rabbitmq consumer
///
/// Arguments:
/// * `settings` - The rabbitmq settings to use for the consumer
/// * `db_settings` - The database settings to use for the consumer
/// * `db_client` - The database client to use for the consumer
/// * `shutdown_rx` - The shutdown receiver to use for the consumer
///
/// Returns:
/// * `AnyhowResult<()>` - The result of the setup
pub async fn setup_rabbitmq(
    settings: &RabbitMQSettings,
    db_pool: &Pool<Postgres>,
    shutdown_rx: tokio::sync::watch::Receiver<()>,
) -> AnyhowResult<()> {
    #[cfg(feature = "rabbitmq")]
    RabbitMQConsumerManager::start_workers(settings, db_pool, shutdown_rx.clone()).await?;
    info!("✅ Started RabbitMQ workers");

    Ok(())
}

/// Helper to setup the background drift worker
/// This worker will continually run and check for drift jobs
/// to run based on their schedules
///
/// Arguments:
/// * `db_client` - The database client to use for the worker
/// * `db_settings` - The database settings to use for the worker
/// * `poll_settings` - The polling settings to use for the worker
/// * `shutdown_rx` - The shutdown receiver to use for the worker
///
/// Returns:
/// * `AnyhowResult<()>` - The result of the setup
pub async fn setup_background_drift_workers(
    db_pool: &Pool<Postgres>,
    poll_settings: &PollingSettings,
    shutdown_rx: tokio::sync::watch::Receiver<()>,
) -> AnyhowResult<()> {
    BackgroundDriftManager::start_workers(db_pool, poll_settings, shutdown_rx).await?;
    info!("✅ Started background workers");

    Ok(())
}

/// Helper to setup the data archiver
/// This worker will continually run and check for expired data based on the retention period
///
/// Arguments:
/// * `db_client` - The database client to use for the worker
/// * `db_settings` - The database settings to use for the worker
/// * `storage_settings` - The storage settings to use for the worker
/// * `shutdown_rx` - The shutdown receiver to use for the worker
///
/// Returns:
/// * `AnyhowResult<()>` - The result of the setup
pub async fn setup_background_data_archive_workers(
    db_pool: &Pool<Postgres>,
    config: &Arc<ScouterServerConfig>,
    shutdown_rx: tokio::sync::watch::Receiver<()>,
) -> AnyhowResult<()> {
    DataArchiver::start_workers(db_pool, shutdown_rx, config).await?;
    info!("✅ Started data archive workers");
    Ok(())
}

#[instrument(skip_all)]
pub async fn setup_components() -> AnyhowResult<(
    Arc<ScouterServerConfig>,
    Pool<Postgres>,
    tokio::sync::watch::Sender<()>,
)> {
    let config = Arc::new(ScouterServerConfig::default());

    // start logging
    let logging = setup_logging().await;
    if logging.is_err() {
        debug!("Failed to setup logging. {:?}", logging.err());
    }

    let db_pool = setup_database(&config.database_settings).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(());

    // if config.kafka_enabled
    if config.kafka_enabled() {
        setup_kafka(
            config.kafka_settings.as_ref().unwrap(),
            &db_pool,
            shutdown_rx.clone(),
        )
        .await?;
    }

    if config.rabbitmq_enabled() {
        setup_rabbitmq(
            config.rabbitmq_settings.as_ref().unwrap(),
            &db_pool,
            shutdown_rx.clone(),
        )
        .await?;
    }

    setup_background_drift_workers(&db_pool, &config.polling_settings, shutdown_rx.clone()).await?;

    setup_background_data_archive_workers(&db_pool, &config, shutdown_rx).await?;

    Ok((config, db_pool, shutdown_tx))
}
