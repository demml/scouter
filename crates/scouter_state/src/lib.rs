pub mod error;
pub use error::StateError;
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use tracing::debug;

use std::future::Future;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create the static current-thread Tokio runtime")
    })
}

/// Blocks on a future using the global current-thread runtime
/// This is primarily used in non-async contexts where we need to call async code
/// from sync code, such as in the ScouterSpanExporter. We use a current-thread
/// runtime to avoid blocking the main thread and causing deadlocks.
pub fn block_on_safe<F: Future>(future: F) -> F::Output {
    get_runtime().block_on(future)
}

//TODO: revisit if we need this struct at all
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
