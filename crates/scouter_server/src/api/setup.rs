use crate::api::archive::DataArchiver;
use crate::api::polling::drift_poller::BackgroundDriftManager;
use crate::api::polling::llm_poller::BackgroundLLMDriftManager;
use anyhow::{Context, Result as AnyhowResult};
use flume::Sender;
use password_auth::generate_hash;
use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger};
use scouter_auth::util::generate_recovery_codes_with_hashes;
use scouter_settings::{
    DatabaseSettings, KafkaSettings, PollingSettings, RabbitMQSettings, ScouterServerConfig,
};
use scouter_sql::sql::traits::UserSqlLogic;
use scouter_sql::sql::{cache::init_entity_cache, cache::EntityCache, schema::User};
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, info, instrument};

#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
use scouter_events::consumer::kafka::{
    consumer::kafka_consumer::create_kafka_consumer, KafkaConsumerManager,
};

#[cfg(feature = "rabbitmq")]
use scouter_events::consumer::rabbitmq::{
    consumer::rabbitmq_consumer::create_rabbitmq_consumer, RabbitMQConsumerManager,
};

#[cfg(feature = "redis_events")]
use scouter_events::consumer::redis::RedisConsumerManager;

use scouter_events::consumer::http::consumer::HttpConsumerManager;
use scouter_settings::events::HttpConsumerSettings;
use scouter_types::MessageRecord;

use crate::api::task_manager::TaskManager;

pub struct ScouterSetupComponents {
    pub server_config: Arc<ScouterServerConfig>,
    pub db_pool: Pool<Postgres>,
    pub task_manager: TaskManager,
    pub http_consumer_tx: Sender<MessageRecord>,
}

impl ScouterSetupComponents {
    pub async fn new() -> AnyhowResult<Self> {
        let config = Arc::new(ScouterServerConfig::new().await);

        // start logging
        let logging = Self::setup_logging().await;
        if logging.is_err() {
            debug!("Failed to setup logging. {:?}", logging.err());
        }

        let db_pool = Self::setup_database(&config.database_settings).await?;
        // entity cache for quick entity id lookups
        let entity_cache = EntityCache::new(db_pool.clone(), 1000);

        let mut task_manager = TaskManager::new();
        let tokio_shutdown_rx = task_manager.get_shutdown_receiver();

        let http_consumer_manager = Self::setup_http_consumer_manager(
            &config.http_consumer_settings,
            &db_pool,
            &mut task_manager,
        )
        .await?;

        // If kafka is enabled, set up the kafka consumer
        if config.kafka_enabled() {
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            Self::setup_kafka(
                config.kafka_settings.as_ref().unwrap(),
                &db_pool,
                &mut task_manager,
            )
            .await?;
        }

        // If rabbitmq is enabled, set up the rabbitmq consumer
        if config.rabbitmq_enabled() {
            #[cfg(feature = "rabbitmq")]
            Self::setup_rabbitmq(
                config.rabbitmq_settings.as_ref().unwrap(),
                &db_pool,
                &mut task_manager,
            )
            .await?;
        }

        // If redis is enabled, set up the redis consumer
        if config.redis_enabled() {
            #[cfg(feature = "redis_events")]
            Self::setup_redis(
                config.redis_settings.as_ref().unwrap(),
                &db_pool,
                tokio_shutdown_rx.clone(),
            )
            .await?;
        }

        // If LLM is enabled, set up the LLM drift workers
        if config.llm_enabled() {
            Self::setup_background_llm_drift_workers(
                &db_pool,
                &config.polling_settings,
                tokio_shutdown_rx.clone(),
            )
            .await?;
        }

        // Set up the background drift workers
        Self::setup_background_drift_workers(
            &db_pool,
            &config.polling_settings,
            tokio_shutdown_rx.clone(),
        )
        .await?;

        Self::setup_background_data_archive_worker(&db_pool, &config, &mut task_manager).await?;

        Ok(Self {
            server_config: config,
            db_pool,
            task_manager,
            http_consumer_tx: http_consumer_manager.tx,
        })
    }

    /// Setup logging for the application
    async fn setup_logging() -> AnyhowResult<()> {
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

    // setup default users
    /// This function is intended to be called the first time the server is started.
    /// If there are no users in the database, it will create defaults.
    /// It is recommended to change the password on first login for these default users.
    /// The default users are:
    /// * admin: admin/admin
    /// * guest: guest/guest
    async fn initialize_default_user(db_pool: &Pool<Postgres>) -> AnyhowResult<()> {
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
        let default_username =
            std::env::var("SCOUTER_DEFAULT_USERNAME").unwrap_or("admin".to_string());
        let default_password =
            std::env::var("SCOUTER_DEFAULT_PASSWORD").unwrap_or("admin".to_string());
        let password_hash = password_auth::generate_hash(&default_password);

        let (_, hashed_recovery_codes) = generate_recovery_codes_with_hashes(1);

        // Create admin user with admin permissions
        let admin_user = User::new(
            default_username.clone(),
            password_hash,
            "admin".to_string(),
            hashed_recovery_codes, // recovery codes
            Some(vec!["read:all".to_string(), "write:all".to_string()]), // permissions
            Some(vec!["admin".to_string()]), // group_permissions
            Some("admin".to_string()), // role
            None,
        );

        // Insert the user
        PostgresClient::insert_user(db_pool, &admin_user)
            .await
            .context("❌ Failed to create default admin user")?;

        let (_, hashed_recovery_codes) = generate_recovery_codes_with_hashes(1);

        // create guest user
        let guest_user = User::new(
            "guest".to_string(),
            generate_hash("guest"),
            "default".to_string(),
            hashed_recovery_codes,
            Some(vec![
                "read:all".to_string(),
                "write:all".to_string(),
                "delete:all".to_string(),
            ]),
            Some(vec!["user".to_string()]),
            Some("guest".to_string()),
            None,
        );
        // Insert the user
        PostgresClient::insert_user(db_pool, &guest_user)
            .await
            .context("❌ Failed to create default guest user")?;

        info!("✅ Created default admin and guest user (change password on first login)",);

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
    async fn setup_background_data_archive_worker(
        db_pool: &Pool<Postgres>,
        config: &Arc<ScouterServerConfig>,
        task_manager: &mut TaskManager,
    ) -> AnyhowResult<()> {
        // Clone needed values for the worker task
        let pool = db_pool.clone();
        let config_clone = config.clone();
        let shutdown_rx = task_manager.get_shutdown_receiver();

        // Spawn the worker directly through the task manager
        task_manager.spawn(DataArchiver::start_worker(
            0,
            pool,
            config_clone,
            shutdown_rx,
        ));

        info!("✅ Started data archive workers");
        Ok(())
    }

    /// Get that database going!
    #[instrument(skip_all)]
    async fn setup_database(db_settings: &DatabaseSettings) -> AnyhowResult<Pool<Postgres>> {
        let db_pool = PostgresClient::create_db_pool(db_settings)
            .await
            .with_context(|| "Failed to create Postgres client")?;

        Self::initialize_default_user(&db_pool).await?;

        // setup entity cache
        init_entity_cache(db_pool.clone(), 1000);

        Ok(db_pool)
    }

    /// Helper to setup the kafka consumer
    ///
    /// Arguments:
    /// * `settings` - The kafka settings to use for the consumer
    /// * `db_settings` - The database settings to use for the consumer
    /// * `db_client` - The database client to use for the consumer
    /// * `task_manager` - The task manager to use for the consumer
    ///
    /// Returns:
    /// * `AnyhowResult<()>` - The result of the setup
    #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
    #[instrument(skip_all)]
    async fn setup_kafka(
        settings: &KafkaSettings,
        db_pool: &Pool<Postgres>,
        task_manager: &mut TaskManager,
    ) -> AnyhowResult<()> {
        let num_consumers = settings.num_workers;

        for id in 0..num_consumers {
            let consumer = create_kafka_consumer(settings, None).await?;
            let kafka_pool = db_pool.clone();
            let shutdown_rx = task_manager.get_shutdown_receiver();

            task_manager.spawn(async move {
                KafkaConsumerManager::start_worker(id, consumer, kafka_pool, shutdown_rx).await;
            });
        }

        debug!("✅ Started {} Kafka workers", num_consumers);

        Ok(())
    }

    /// Helper to setup the rabbitmq consumer
    ///
    /// Arguments:
    /// * `settings` - The rabbitmq settings to use for the consumer
    /// * `db_settings` - The database settings to use for the consumer
    /// * `db_client` - The database client to use for the consumer
    /// * `task_manager` - The task manager to use for the consumer
    ///
    /// Returns:
    /// * `AnyhowResult<()>` - The result of the setup
    #[cfg(feature = "rabbitmq")]
    #[instrument(skip_all)]
    async fn setup_rabbitmq(
        settings: &RabbitMQSettings,
        db_pool: &Pool<Postgres>,
        task_manager: &mut TaskManager,
    ) -> AnyhowResult<()> {
        let num_consumers = settings.num_consumers;

        for id in 0..num_consumers {
            let consumer = create_rabbitmq_consumer(settings).await?;
            let rabbit_db_pool = db_pool.clone();
            let shutdown_rx = task_manager.get_shutdown_receiver();

            task_manager.spawn(async move {
                RabbitMQConsumerManager::start_worker(id, consumer, rabbit_db_pool, shutdown_rx)
                    .await;
            });
        }

        info!("✅ Started {} RabbitMQ workers", num_consumers);

        Ok(())
    }

    /// Helper to set up the default http consumer used in the absense of Kafka and Rabbitmq
    ///
    /// Arguments:
    /// * `settings` - The http consumer settings
    /// * `db_pool` - The pg db pool used by the consumers
    /// * `task_manager` - The task manager to use for the consumer
    ///
    /// Returns:
    /// * `AnyhowResult<HttpConsumerManager>` - http consumer manager struct containing the flume channel transmitter
    #[instrument(skip_all)]
    async fn setup_http_consumer_manager(
        settings: &HttpConsumerSettings,
        db_pool: &Pool<Postgres>,

        task_manager: &mut TaskManager,
    ) -> AnyhowResult<HttpConsumerManager> {
        let (tx, rx) = flume::bounded(1000);
        let num_workers = settings.num_workers;

        for id in 0..num_workers {
            let consumer = rx.clone();
            let worker_shutdown_rx = task_manager.get_shutdown_receiver();
            let db_pool_clone = db_pool.clone();

            task_manager.spawn(async move {
                HttpConsumerManager::start_worker(id, consumer, db_pool_clone, worker_shutdown_rx)
                    .await;
            });
        }

        info!("✅ Started {} HTTP consumers", num_workers);
        Ok(HttpConsumerManager { tx })
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
    #[instrument(skip_all)]
    async fn setup_background_drift_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &PollingSettings,
        shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> AnyhowResult<()> {
        BackgroundDriftManager::start_workers(db_pool, poll_settings, shutdown_rx).await?;
        info!("✅ Started background workers");

        Ok(())
    }

    /// Helper to setup the background LLM drift worker
    /// This worker will continually run and check if there are any drift records to process
    /// and will run the LLM drift evaluation jobs
    ///
    /// Arguments:
    /// * `db_client` - The database client to use for the worker
    /// * `db_settings` - The database settings to use for the worker
    /// * `poll_settings` - The polling settings to use for the worker
    /// * `shutdown_rx` - The shutdown receiver to use for the worker
    ///
    /// Returns:
    /// * `AnyhowResult<()>` - The result of the setup
    #[instrument(skip_all)]
    async fn setup_background_llm_drift_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &PollingSettings,

        shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> AnyhowResult<()> {
        BackgroundLLMDriftManager::start_workers(db_pool, poll_settings, shutdown_rx).await?;
        info!("✅ Started background llm workers");

        Ok(())
    }

    /// Helper to setup the redis consumer
    /// This worker will continually run and check for redis events
    ///
    /// Arguments:
    /// * `settings` - The redis settings to use for the consumer
    /// * `db_pool` - The database client to use for the worker
    /// * `shutdown_rx` - The shutdown receiver to use for the worker
    ///
    /// Returns:
    /// * `AnyhowResult<()>` - The result of the setup
    #[cfg(feature = "redis_events")]
    #[instrument(skip_all)]
    pub async fn setup_redis(
        settings: &scouter_settings::RedisSettings,
        db_pool: &Pool<Postgres>,

        shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> AnyhowResult<()> {
        RedisConsumerManager::start_workers(settings, db_pool, shutdown_rx).await?;
        info!("✅ Started Redis workers");

        Ok(())
    }
}
