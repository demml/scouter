// Module to process GenAI drift record tasks
use crate::api::error::ServerError;
use scouter_drift::genai::AgentPoller;
use scouter_settings::polling::AgentPollerSettings;
use sqlx::{Pool, Postgres};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, span, Instrument, Level};

pub struct BackgroundAgentDriftManager {
    pub workers: Vec<JoinHandle<()>>,
}

impl BackgroundAgentDriftManager {
    pub async fn start_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &AgentPollerSettings,
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

            workers.push(tokio::spawn(Self::start_worker(
                id,
                agent_poller,
                worker_shutdown_rx,
            )));

            // sleep for a bit to stagger the start of the workers
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        debug!("✅ Started {} drift workers", num_workers);

        Ok(())
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
