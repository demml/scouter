use crate::error::DriftError;
use scouter_dataframe::parquet::tracing::queries::TraceQueries;
use scouter_sql::PostgresClient;
use scouter_sql::sql::error::SqlError;
use scouter_sql::sql::traits::AgentDriftSqlLogic;
use scouter_types::TraceCommitAnchor;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

const EVENT_WORKER_INTERVAL: Duration = Duration::from_secs(1);
const EVENT_BATCH_LIMIT: i64 = 500;
const AWAITING_TRACE_TIMEOUT: chrono::Duration = chrono::Duration::minutes(5);
const TRACE_COMMIT_EVENT_RETENTION: chrono::Duration = chrono::Duration::days(1);
const COMMIT_INSERT_INITIAL_BACKOFF: Duration = Duration::from_millis(100);
const COMMIT_INSERT_MAX_BACKOFF: Duration = Duration::from_secs(5);
const RECONCILE_BATCH_LIMIT: i64 = 200;
const RECONCILE_LOOKBACK_SECS: i64 = 86_400;

#[derive(Clone, Debug)]
pub struct InboxSweepConfig {
    pub timeout_interval: Duration,
    pub prune_interval: Duration,
    pub reconcile_interval: Duration,
    pub lease_recovery_interval: Duration,
    pub processed_retention: chrono::Duration,
    pub trace_arrival_timeout: chrono::Duration,
    pub reconcile_after: chrono::Duration,
    pub reconcile_lookback: chrono::Duration,
    pub lease_ttl: chrono::Duration,
    pub max_attempts: i32,
}

impl Default for InboxSweepConfig {
    fn default() -> Self {
        Self {
            timeout_interval: Duration::from_secs(60),
            prune_interval: Duration::from_secs(60),
            reconcile_interval: Duration::from_secs(
                env_u64("SCOUTER_INBOX_RECONCILE_INTERVAL_SECS", 60).max(1),
            ),
            lease_recovery_interval: Duration::from_secs(60),
            processed_retention: TRACE_COMMIT_EVENT_RETENTION,
            trace_arrival_timeout: AWAITING_TRACE_TIMEOUT,
            reconcile_after: chrono::Duration::seconds(env_i64(
                "SCOUTER_INBOX_RECONCILE_AFTER_SECS",
                15,
            )),
            reconcile_lookback: chrono::Duration::seconds(env_i64(
                "SCOUTER_INBOX_RECONCILE_LOOKBACK_SECS",
                RECONCILE_LOOKBACK_SECS,
            )),
            lease_ttl: chrono::Duration::minutes(2),
            max_attempts: 5,
        }
    }
}

fn env_i64(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

pub async fn run_commit_consumer_loop(
    pool: Pool<Postgres>,
    mut rx: mpsc::Receiver<Vec<TraceCommitAnchor>>,
    mut shutdown: watch::Receiver<()>,
) {
    info!("trace-anchor commit consumer started");
    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!("trace-anchor commit consumer shutting down");
                break;
            }
            maybe_anchors = rx.recv() => {
                let Some(anchors) = maybe_anchors else { break };
                if anchors.is_empty() {
                    continue;
                }
                insert_trace_commit_events_with_retry(&pool, &anchors, &mut shutdown).await;
            }
        }
    }
}

async fn insert_trace_commit_events_with_retry(
    pool: &Pool<Postgres>,
    anchors: &[TraceCommitAnchor],
    shutdown: &mut watch::Receiver<()>,
) -> bool {
    let mut backoff = COMMIT_INSERT_INITIAL_BACKOFF;

    loop {
        match PostgresClient::insert_trace_commit_events(pool, anchors).await {
            Ok(result) => {
                let inserted = result.rows_affected() as usize;
                let duplicate = anchors.len().saturating_sub(inserted);
                if duplicate > 0 {
                    metrics::counter!("scouter_trace_commit_event_duplicate_total")
                        .increment(duplicate as u64);
                }
                return true;
            }
            Err(e) => {
                if is_permanent_inbox_insert_error(&e) {
                    metrics::counter!("scouter_trace_commit_event_invalid_total")
                        .increment(anchors.len() as u64);
                    error!(
                        error = ?e,
                        anchor_count = anchors.len(),
                        "insert_trace_commit_events rejected permanent invalid anchor batch; dropping"
                    );
                    return false;
                }

                error!(
                    error = ?e,
                    anchor_count = anchors.len(),
                    backoff_ms = backoff.as_millis(),
                    "insert_trace_commit_events failed; retrying"
                );
            }
        }

        tokio::select! {
            _ = shutdown.changed() => {
                warn!(
                    anchor_count = anchors.len(),
                    "trace-anchor consumer shutting down before commit events were inserted"
                );
                return false;
            }
            _ = tokio::time::sleep(backoff) => {
                backoff = std::cmp::min(backoff.saturating_mul(2), COMMIT_INSERT_MAX_BACKOFF);
            }
        }
    }
}

fn is_permanent_inbox_insert_error(error: &SqlError) -> bool {
    let SqlError::SqlxError(sqlx::Error::Database(db)) = error else {
        return false;
    };

    matches!(
        db.code().as_deref(),
        Some("22001" | "23502" | "23514" | "22P02")
    )
}

pub async fn run_trace_commit_event_worker_loop(
    pool: Pool<Postgres>,
    trace_visibility_buffer: chrono::Duration,
    trace_query: Arc<TraceQueries>,
    mut shutdown: watch::Receiver<()>,
) {
    let mut tick = tokio::time::interval(EVENT_WORKER_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sweeps = Box::pin(run_sweeps(
        pool.clone(),
        trace_query,
        InboxSweepConfig::default(),
        shutdown.clone(),
    ));
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
            _ = &mut sweeps => {
                info!("trace-commit sweep manager exited");
                break;
            }
            _ = tick.tick() => {
                if let Err(e) = drain_once(&pool, trace_visibility_buffer, "trace-inbox-worker", 5).await {
                    warn!("event worker drain failed: {:?}", e);
                }
            }
        }
    }
}

pub(crate) async fn drain_once(
    pool: &Pool<Postgres>,
    trace_visibility_buffer: chrono::Duration,
    worker_id: &str,
    max_attempts: i32,
) -> Result<(), DriftError> {
    loop {
        let claim_started = Instant::now();
        let claimed =
            PostgresClient::claim_trace_commit_events(pool, EVENT_BATCH_LIMIT, worker_id).await?;
        metrics::histogram!("scouter_trace_commit_event_claim_latency_ms")
            .record(claim_started.elapsed().as_secs_f64() * 1000.0);
        if claimed.is_empty() {
            return Ok(());
        }

        let claim_token = claimed[0].claim_token;
        let event_ids: Vec<i64> = claimed.iter().map(|claim| claim.id).collect();

        let flip_result = async {
            let mut tx = pool.begin().await.map_err(SqlError::from)?;
            let completed =
                PostgresClient::complete_trace_commit_events(&mut tx, &event_ids, claim_token)
                    .await?;
            let flipped = if completed.is_empty() {
                0
            } else {
                PostgresClient::flip_awaiting_evals(&mut tx, &completed, trace_visibility_buffer)
                    .await?
                    .rows_affected()
            };
            tx.commit().await.map_err(SqlError::from)?;
            Ok::<(u64, usize), DriftError>((flipped, completed.len()))
        }
        .await;

        match flip_result {
            Ok((flipped, completed)) => {
                let fenced = event_ids.len().saturating_sub(completed);
                if fenced > 0 {
                    metrics::counter!("scouter_trace_commit_event_complete_fenced_total")
                        .increment(fenced as u64);
                    warn!(
                        fenced,
                        batch = event_ids.len(),
                        "complete_trace_commit_events fenced by stale claim_token"
                    );
                }
                metrics::counter!("scouter_trace_commit_event_processed_total")
                    .increment(completed as u64);

                if completed > 0 && flipped == 0 {
                    metrics::counter!("scouter_trace_commit_event_processed_noop_total")
                        .increment(completed as u64);
                    debug!(
                        completed,
                        "drained anchor batch; eval rows already terminal or missing"
                    );
                } else if flipped > 0 {
                    metrics::histogram!("scouter_trace_commit_event_flip_rows_affected")
                        .record(flipped as f64);
                    debug!(completed, flipped, "drained trace-anchor commit batch");
                }
            }
            Err(e) => {
                let message = format!("{e:?}");
                match PostgresClient::fail_trace_commit_events(
                    pool,
                    &event_ids,
                    max_attempts,
                    &message,
                    claim_token,
                )
                .await
                {
                    Ok(rows) => {
                        let returned = rows
                            .iter()
                            .filter(|(_, status)| status == "pending")
                            .count();
                        let dead_lettered = rows
                            .iter()
                            .filter(|(_, status)| status == "dead_lettered")
                            .count();
                        let fenced = event_ids.len().saturating_sub(returned + dead_lettered);
                        metrics::counter!("scouter_trace_commit_event_lease_recovered_total")
                            .increment(returned as u64);
                        metrics::counter!("scouter_trace_commit_event_dead_lettered_total")
                            .increment(dead_lettered as u64);
                        if fenced > 0 {
                            metrics::counter!("scouter_trace_commit_event_fail_fenced_total")
                                .increment(fenced as u64);
                            warn!(
                                fenced,
                                batch = event_ids.len(),
                                "fail_trace_commit_events fenced by stale claim_token"
                            );
                        }
                    }
                    Err(release_err) => {
                        error!(
                            ?release_err,
                            "fail_trace_commit_events also failed; lease recovery sweep will retry"
                        );
                    }
                }
                return Err(e);
            }
        }

        if (claimed.len() as i64) < EVENT_BATCH_LIMIT {
            return Ok(());
        }
    }
}

pub(crate) async fn reconcile_lost_events(
    pool: &Pool<Postgres>,
    trace_query: &TraceQueries,
    reconcile_after: chrono::Duration,
    lookback: chrono::Duration,
    trace_arrival_timeout: chrono::Duration,
) -> Result<usize, DriftError> {
    let batch_limit = env_i64("SCOUTER_INBOX_RECONCILE_BATCH", RECONCILE_BATCH_LIMIT);
    let stuck =
        PostgresClient::list_awaiting_record_uids(pool, reconcile_after, batch_limit).await?;
    if stuck.is_empty() {
        return Ok(0);
    }

    let confirmed = trace_query
        .find_anchor_spans_for_records(&stuck, lookback, trace_arrival_timeout)
        .await
        .map_err(|e| DriftError::RunTimeError(e.to_string()))?;
    if confirmed.is_empty() {
        return Ok(0);
    }

    let result = PostgresClient::insert_trace_commit_events(pool, &confirmed).await?;
    let inserted = result.rows_affected() as usize;
    let duplicate = confirmed.len().saturating_sub(inserted);
    if duplicate > 0 {
        metrics::counter!("scouter_trace_commit_event_duplicate_total").increment(duplicate as u64);
    }
    metrics::counter!("scouter_trace_commit_event_reconciled_total").increment(inserted as u64);
    info!(
        recovered = inserted,
        scanned = stuck.len(),
        "reconciliation sweep recovered lost anchor events"
    );
    Ok(inserted)
}

pub async fn run_sweeps(
    pool: Pool<Postgres>,
    trace_query: Arc<TraceQueries>,
    cfg: InboxSweepConfig,
    mut shutdown: watch::Receiver<()>,
) {
    let mut timeout_tick = tokio::time::interval(cfg.timeout_interval);
    let mut prune_tick = tokio::time::interval(cfg.prune_interval);
    let mut reconcile_tick = tokio::time::interval(cfg.reconcile_interval);
    let mut lease_tick = tokio::time::interval(cfg.lease_recovery_interval);

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            _ = timeout_tick.tick() => {
                match PostgresClient::sweep_awaiting_trace_timeouts(&pool, cfg.trace_arrival_timeout).await {
                    Ok(result) if result.rows_affected() > 0 => warn!(
                        timed_out = result.rows_affected(),
                        "TraceArrivalTimeout sweep failed rows"
                    ),
                    Ok(_) => {}
                    Err(e) => warn!("timeout sweep failed: {:?}", e),
                }
            }
            _ = prune_tick.tick() => {
                if let Err(e) = PostgresClient::prune_processed_events(&pool, cfg.processed_retention).await {
                    warn!("prune sweep failed: {:?}", e);
                }
            }
            _ = reconcile_tick.tick() => {
                if let Err(e) = reconcile_lost_events(
                    &pool,
                    &trace_query,
                    cfg.reconcile_after,
                    cfg.reconcile_lookback,
                    cfg.trace_arrival_timeout,
                ).await {
                    warn!("reconciliation sweep failed: {:?}", e);
                }
            }
            _ = lease_tick.tick() => {
                refresh_queue_gauges(&pool).await;
                match PostgresClient::recover_stale_processing(&pool, cfg.lease_ttl, cfg.max_attempts).await {
                    Ok(rows) => {
                        let returned = rows.iter().filter(|(_, status)| status == "pending").count();
                        let dead_lettered = rows
                            .iter()
                            .filter(|(_, status)| status == "dead_lettered")
                            .count();
                        metrics::counter!("scouter_trace_commit_event_lease_recovered_total")
                            .increment(returned as u64);
                        metrics::counter!("scouter_trace_commit_event_dead_lettered_total")
                            .increment(dead_lettered as u64);
                    }
                    Err(e) => warn!("recover_stale_processing failed: {:?}", e),
                }
            }
        }
    }
}

#[cfg(any(test, feature = "test-helpers"))]
pub(crate) async fn run_basic_sweeps_once(pool: &Pool<Postgres>) {
    if let Err(e) =
        PostgresClient::sweep_awaiting_trace_timeouts(pool, AWAITING_TRACE_TIMEOUT).await
    {
        warn!("timeout sweep failed: {:?}", e);
    }

    if let Err(e) = PostgresClient::prune_processed_events(pool, TRACE_COMMIT_EVENT_RETENTION).await
    {
        warn!("prune sweep failed: {:?}", e);
    }

    match PostgresClient::recover_stale_processing(pool, chrono::Duration::minutes(2), 5).await {
        Ok(rows) => {
            let returned = rows
                .iter()
                .filter(|(_, status)| status == "pending")
                .count();
            let dead_lettered = rows
                .iter()
                .filter(|(_, status)| status == "dead_lettered")
                .count();
            metrics::counter!("scouter_trace_commit_event_lease_recovered_total")
                .increment(returned as u64);
            metrics::counter!("scouter_trace_commit_event_dead_lettered_total")
                .increment(dead_lettered as u64);
        }
        Err(e) => warn!("recover_stale_processing failed: {:?}", e),
    }
}

async fn refresh_queue_gauges(pool: &Pool<Postgres>) {
    let row = sqlx::query(
        r#"
        SELECT count(*)::bigint AS pending_count,
               COALESCE(EXTRACT(EPOCH FROM now() - min(created_at)), 0)::double precision
                   AS oldest_age_seconds
        FROM scouter.trace_commit_event
        WHERE status = 'pending'
        "#,
    )
    .fetch_one(pool)
    .await;

    match row {
        Ok(row) => {
            use sqlx::Row;
            let pending_count: i64 = row.try_get("pending_count").unwrap_or(0);
            let oldest_age_seconds: f64 = row.try_get("oldest_age_seconds").unwrap_or(0.0);
            metrics::gauge!("scouter_trace_commit_event_pending_count").set(pending_count as f64);
            metrics::gauge!("scouter_trace_commit_event_oldest_pending_age_seconds")
                .set(oldest_age_seconds);
        }
        Err(e) => warn!("trace commit queue gauge refresh failed: {:?}", e),
    }
}
