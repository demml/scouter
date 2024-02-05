pub struct Logger {}

impl Logger {
    pub fn new() -> Logger {
        tracing_subscriber::fmt()
            .json()
            .with_target(false)
            .with_max_level(tracing::Level::TRACE)
            .with_current_span(false)
            .init();
        Logger {}
    }

    pub fn info(self, message: &str) -> () {
        tracing::info!("{}", message);
    }

    pub fn debug(self, message: &str) -> () {
        tracing::debug!("{}", message);
    }

    pub fn warn(self, message: &str) -> () {
        tracing::warn!("{}", message);
    }

    pub fn error(self, message: &str) -> () {
        tracing::error!("{}", message);
    }
}
