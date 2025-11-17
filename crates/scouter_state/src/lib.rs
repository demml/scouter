pub mod error;
pub use error::StateError;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use tokio::runtime::{Handle, Runtime};
use tracing::debug;

/// Manages the application's global Tokio runtime.
pub struct ScouterState {
    pub runtime: Arc<Runtime>,
}

impl ScouterState {
    /// Creates a new multi-threaded Tokio runtime.
    fn new() -> Result<Self, StateError> {
        debug!("Initializing ScouterState (Multi-threaded Runtime)");
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread() // Multi-thread is generally better for background work
                .enable_all()
                .build()
                .map_err(StateError::RuntimeError)?,
        );
        Ok(Self { runtime })
    }

    /// Provides a Handle to the global runtime.
    pub fn handle(&self) -> Handle {
        self.runtime.handle().clone()
    }

    /// Blocks on a future using this runtime. This is safe because it uses a dedicated
    /// multi-threaded runtime, isolating it from the main application's event loop.
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}

// Global instance of the application state manager
static INSTANCE: OnceLock<ScouterState> = OnceLock::new();

/// Global accessor for the application state, ensuring the runtime is initialized once.
pub fn app_state() -> &'static ScouterState {
    INSTANCE.get_or_init(|| ScouterState::new().expect("Failed to initialize state"))
}

/// A safe utility function to block on an async future using the global multi-threaded runtime.
pub fn block_on<F: Future>(future: F) -> F::Output {
    app_state().block_on(future)
}
