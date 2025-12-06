use pyo3::prelude::*;
use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger, WriteLevel};
use std::env;
use std::str::FromStr;

#[pyfunction]
fn _get_log_level() -> LogLevel {
    LogLevel::from_str(&env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()))
        .unwrap_or(LogLevel::Info)
}

#[pyfunction]
fn _log_json() -> bool {
    env::var("LOG_JSON").unwrap_or_else(|_| "false".to_string()) == "true"
}

pub fn add_logging_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<LogLevel>()?;
    m.add_class::<RustyLogger>()?;
    m.add_class::<LoggingConfig>()?;
    m.add_class::<WriteLevel>()?;
    m.add_function(wrap_pyfunction!(_get_log_level, m)?)?;
    m.add_function(wrap_pyfunction!(_log_json, m)?)?;

    Ok(())
}
