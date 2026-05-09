use crate::error::DriftError;
use scouter_sql::PostgresClient;
use scouter_sql::sql::error::SqlError;
use scouter_sql::sql::traits::AgentDriftSqlLogic;
use scouter_types::TraceId;
use sqlx::{Pool, Postgres};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

const EVENT_WORKER_INTERVAL: Duration = Duration::from_secs(1);
const EVENT_BATCH_LIMIT: i64 = 500;
const SWEEP_TICKS: u32 = 60;
const AWAITING_TRACE_TIMEOUT: chrono::Duration = chrono::Duration::minutes(5);
const TRACE_COMMIT_EVENT_RETENTION: chrono::Duration = chrono::Duration::days(1);
const COMMIT_INSERT_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const COMMIT_INSERT_MAX_BACKOFF: Duration = Duration::from_secs(5);

pub async fn run_commit_consumer_loop(
    pool: Pool<Postgres>,
    mut rx: mpsc::Receiver<Vec<TraceId>>,
    mut shutdown: watch::Receiver<()>,
) {
    info!("trace-commit consumer started");
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("trace-commit consumer shutting down");
                break;
            }
            maybe_ids = rx.recv() => {
                let Some(trace_ids) = maybe_ids else { break };
                if trace_ids.is_empty() {
                    continue;
                }
                insert_trace_commit_events_with_retry(&pool, &trace_ids, &mut shutdown).await;
            }
        }
    }
}

async fn insert_trace_commit_events_with_retry(
    pool: &Pool<Postgres>,
    trace_ids: &[TraceId],
    shutdown: &mut watch::Receiver<()>,
) -> bool {
    let mut backoff = COMMIT_INSERT_INITIAL_BACKOFF;

    loop {
        match PostgresClient::insert_trace_commit_events(pool, trace_ids).await {
            Ok(_) => return true,
            Err(e) => {
                error!(
                    error = ?e,
                    trace_count = trace_ids.len(),
                    backoff_ms = backoff.as_millis(),
                    "insert_trace_commit_events failed; retrying"
                );
            }
        }

        tokio::select! {
            _ = shutdown.changed() => {
                warn!(
                    trace_count = trace_ids.len(),
                    "trace-commit consumer shutting down before commit events were inserted"
                );
                return false;
            }
            _ = tokio::time::sleep(backoff) => {
                backoff = std::cmp::min(backoff.saturating_mul(2), COMMIT_INSERT_MAX_BACKOFF);
            }
        }
    }
}

pub async fn run_trace_commit_event_worker_loop(
    pool: Pool<Postgres>,
    trace_visibility_buffer: chrono::Duration,
    mut shutdown: watch::Receiver<()>,
) {
    let mut tick = tokio::time::interval(EVENT_WORKER_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut counter: u32 = 0;
    info!(
        "trace-commit event worker started (interval = {:?})",
        EVENT_WORKER_INTERVAL
    );

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("trace-commit event worker shutting down");
                break;
            }
            _ = tick.tick() => {
                if let Err(e) = drain_once(&pool, trace_visibility_buffer).await {
                    warn!("event worker drain failed: {:?}", e);
                }
                counter = counter.wrapping_add(1);
                if counter.is_multiple_of(SWEEP_TICKS) {
                    run_sweeps(&pool).await;
                }
            }
        }
    }
}

pub(crate) async fn drain_once(
    pool: &Pool<Postgres>,
    trace_visibility_buffer: chrono::Duration,
) -> Result<(), DriftError> {
    loop {
        let mut tx = pool.begin().await.map_err(SqlError::from)?;
        let claimed =
            match PostgresClient::claim_trace_commit_events(&mut tx, EVENT_BATCH_LIMIT).await {
                Ok(claimed) => claimed,
                Err(e) => {
                    tx.rollback().await.ok();
                    return Err(e.into());
                }
            };

        if claimed.is_empty() {
            tx.rollback().await.map_err(SqlError::from)?;
            return Ok(());
        }

        let event_ids: Vec<i64> = claimed.iter().map(|(id, _)| *id).collect();
        let trace_ids: Vec<TraceId> = claimed.iter().map(|(_, trace_id)| *trace_id).collect();

        let flipped =
            PostgresClient::flip_awaiting_evals(&mut tx, &trace_ids, trace_visibility_buffer)
                .await?;
        let _ = PostgresClient::mark_events_processed(&mut tx, &event_ids).await?;
        tx.commit().await.map_err(SqlError::from)?;

        debug!(
            claimed = claimed.len(),
            flipped = flipped.rows_affected(),
            "drained trace-commit event batch"
        );

        if (claimed.len() as i64) < EVENT_BATCH_LIMIT {
            return Ok(());
        }
    }
}

pub(crate) async fn run_sweeps(pool: &Pool<Postgres>) {
    match PostgresClient::sweep_awaiting_trace_timeouts(pool, AWAITING_TRACE_TIMEOUT).await {
        Ok(result) if result.rows_affected() > 0 => warn!(
            timed_out = result.rows_affected(),
            "TraceArrivalTimeout sweep failed rows"
        ),
        Ok(_) => {}
        Err(e) => warn!("timeout sweep failed: {:?}", e),
    }

    if let Err(e) = PostgresClient::prune_processed_events(pool, TRACE_COMMIT_EVENT_RETENTION).await
    {
        warn!("prune sweep failed: {:?}", e);
    }
}
