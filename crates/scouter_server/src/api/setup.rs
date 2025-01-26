use std::str::FromStr;

use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger};
use tracing::info;

/// Setup logging for the application
///
/// This function initializes the logging system for the application
pub async fn setup_logging() -> Result<(), anyhow::Error> {
    let log_level = LogLevel::from_str(
        std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string())
            .as_str(),
    )?;
    let config = LoggingConfig::new(Some(true), Some(log_level), None, Some(true));
    RustyLogger::setup_logging(Some(config))?;

    info!("Logging setup successfully");

    Ok(())
}
