// Module to process GenAI drift record tasks
use crate::api::error::ServerError;
use scouter_dataframe::parquet::tracing::queries::TraceQueries;
use scouter_drift::genai::AgentPoller;
use scouter_settings::polling::AgentPollerSettings;
use scouter_types::TraceCommitAnchor;
use sqlx::{Pool, Postgres};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tracing::{Instrument, Level, debug, error, info, span};

pub struct BackgroundAgentDriftManager {
    pub workers: Vec<JoinHandle<()>>,
}

impl BackgroundAgentDriftManager {
    pub async fn start_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &AgentPollerSettings,
        commit_rx: mpsc::Receiver<Vec<TraceCommitAnchor>>,
        trace_query: Arc<TraceQueries>,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ServerError> {
        let num_workers = poll_settings.genai_workers;
        info!("Starting {} agent eval workers", num_workers);
        let mut workers = Vec::with_capacity(num_workers);

        for id in 0..num_workers {
            let shutdown_rx = shutdown_rx.clone();
            let agent_poller = AgentPoller::new(
                db_pool,
                poll_settings.max_retries,
                poll_settings.trace_wait_timeout,
                poll_settings.trace_backoff,
                poll_settings.trace_reschedule_delay,
            );
            let worker_shutdown_rx = shutdown_rx.clone();

            workers.push(Self::spawn_monitored_worker(
                "agent eval worker",
                Self::start_worker(id, agent_poller, worker_shutdown_rx),
                shutdown_rx.clone(),
            ));

            // sleep for a bit to stagger the start of the workers
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        debug!("✅ Started {} drift workers", num_workers);

        Self::spawn_monitored_worker(
            "trace-commit consumer",
            scouter_drift::genai::inbox::run_commit_consumer_loop(
                db_pool.clone(),
                commit_rx,
                shutdown_rx.clone(),
            ),
            shutdown_rx.clone(),
        );

        Self::spawn_monitored_worker(
            "trace-commit event worker",
            scouter_drift::genai::inbox::run_trace_commit_event_worker_loop(
                db_pool.clone(),
                poll_settings.trace_visibility_buffer,
                trace_query,
                shutdown_rx.clone(),
            ),
            shutdown_rx.clone(),
        );

        Ok(())
    }

    fn spawn_monitored_worker<F>(
        name: &'static str,
        future: F,
        shutdown: watch::Receiver<()>,
    ) -> JoinHandle<()>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(async move {
            future.await;
            if shutdown.has_changed().unwrap_or(true) {
                info!("{name} exited after shutdown signal");
            } else {
                error!("{name} exited unexpectedly");
            }
        })
    }

    async fn start_worker(
        id: usize,
        mut poller: AgentPoller,
        mut shutdown: watch::Receiver<()>, // Accept receiver
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Agent evaluator {}: Shutting down", id);
                    break;
                }
                result = poller.poll_for_tasks().instrument(span!(Level::INFO, "poll_for_agent_tasks")) => {
                    if let Err(e) = result {
                        error!("Alert poller error: {:?}", e);
                    }
                }
            }
        }
    }
}
