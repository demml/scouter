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
        let filter =
            EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info"));

        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(true)
            .with_line_number(true);

        // Use try_init() instead of init() to avoid panicking if already initialized
        if let Err(e) = tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .try_init()
        {
            // Log the error but don't panic - subscriber is already set
            eprintln!("Warning: tracing subscriber already initialized: {}", e);
        } else {
            tracing::debug!("Tracing initialized successfully");
        }
    });
}

pub use mock::ScouterTestServer;
pub use potato_head::mock::LLMTestServer;
pub use util::{
    create_multi_service_trace, create_nested_trace, create_sequence_pattern_trace,
    create_simple_trace, create_simple_trace_no_py, create_trace_with_attributes,
    create_trace_with_errors,
};
