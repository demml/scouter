pub mod error;
pub use error::StateError;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tracing::debug;

pub struct ScouterState {
    pub runtime: Arc<Runtime>,
}

impl ScouterState {
    fn new() -> Result<Self, StateError> {
        debug!("Initializing ScouterState");
        let runtime = Arc::new(Runtime::new().map_err(StateError::RuntimeError)?);
        Ok(Self { runtime })
    }

    pub fn start_runtime(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }
}

// Global instance
static INSTANCE: OnceLock<ScouterState> = OnceLock::new();

// Global accessor
/// ScouterState is primarily used as a global singleton to manage the Tokio runtime for python code that
/// needs to call async Rust code. This is because Python's GIL does not play well with Rust's async model, and we don't
/// want to create a new runtime for every call and we want to avoid blocking the main thread, which would
/// cause deadlocks and performance issues.
pub fn app_state() -> &'static ScouterState {
    INSTANCE.get_or_init(|| ScouterState::new().expect("Failed to initialize state"))
}
