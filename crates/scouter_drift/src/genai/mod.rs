#[cfg(feature = "sql")]
pub mod poller;

#[cfg(feature = "sql")]
pub mod drift;

#[cfg(feature = "sql")]
pub mod inbox;

#[cfg(feature = "sql")]
pub use drift::AgentDrifter;

#[cfg(feature = "sql")]
pub use poller::AgentPoller;

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers {
    use crate::error::DriftError;
    use sqlx::{Pool, Postgres};

    use super::inbox;

    pub async fn drain_once(pool: &Pool<Postgres>) -> Result<(), DriftError> {
        inbox::drain_once(pool, chrono::Duration::zero()).await
    }

    pub async fn run_sweeps(pool: &Pool<Postgres>) {
        inbox::run_sweeps(pool).await;
    }
}
