pub mod mock;
pub mod util;

#[cfg(feature = "server")]
use std::sync::Once;
#[cfg(feature = "server")]
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[cfg(feature = "server")]
static TRACING_INIT: Once = Once::new();

#[cfg(feature = "server")]
pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        // 1. Build the filter from the "LOG_LEVEL" environment variable
        // If the variable isn't set, it defaults to "info"
        let filter =
            EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info"));

        // 2. Configure the formatting layer
        let fmt_layer = fmt::layer()
            .with_target(true) // Include the module path
            .with_thread_ids(true) // Useful for debugging async/concurrent code
            .with_line_number(true);

        // 3. Initialize the global subscriber
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();

        tracing::debug!("Tracing initialized successfully");
    });
}

pub use mock::ScouterTestServer;
pub use potato_head::mock::LLMTestServer;
pub use util::{
    create_multi_service_trace, create_nested_trace, create_sequence_pattern_trace,
    create_simple_trace, create_trace_with_attributes, create_trace_with_errors,
};
