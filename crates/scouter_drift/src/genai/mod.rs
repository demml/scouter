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
    use scouter_dataframe::parquet::tracing::queries::TraceQueries;
    use sqlx::{Pool, Postgres};

    use super::inbox;

    pub async fn drain_once(pool: &Pool<Postgres>) -> Result<(), DriftError> {
        inbox::drain_once(pool, chrono::Duration::zero(), "test-worker", 5).await
    }

    pub async fn run_sweeps(pool: &Pool<Postgres>) {
        inbox::run_basic_sweeps_once(pool).await;
    }

    pub async fn reconcile_lost_events(
        pool: &Pool<Postgres>,
        trace_query: &TraceQueries,
    ) -> Result<usize, DriftError> {
        reconcile_lost_events_with_lookback(pool, trace_query, chrono::Duration::days(1)).await
    }

    pub async fn reconcile_lost_events_with_lookback(
        pool: &Pool<Postgres>,
        trace_query: &TraceQueries,
        lookback: chrono::Duration,
    ) -> Result<usize, DriftError> {
        inbox::reconcile_lost_events(
            pool,
            trace_query,
            chrono::Duration::zero(),
            lookback,
            chrono::Duration::minutes(5),
        )
        .await
    }
}
