use std::sync::Arc;

use crate::error::{EventError, PyEventError};
use pyo3::prelude::*;
use scouter_types::QueueItem;
use std::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, warn};

#[derive(Debug)]
pub enum Event {
    Task(QueueItem),
    Flush,
}

#[derive(Debug)]
pub struct Task {
    pub abort_handle: Option<AbortHandle>,
    pub running: bool,
    pub cancel_token: Option<CancellationToken>,
}

impl Task {
    pub fn new() -> Self {
        Self {
            abort_handle: None,
            running: false,
            cancel_token: None,
        }
    }
}

impl Default for Task {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TaskState {
    // track the task that receives events
    pub event_task: Arc<RwLock<Task>>,

    // track the task that processes background tasks (only applies to psi and custom)
    pub background_task: Arc<RwLock<Task>>,

    // channel to send events to the event task
    pub event_tx: UnboundedSender<Event>,

    pub id: String,
}

impl TaskState {
    pub fn add_background_cancellation_token(&mut self, token: CancellationToken) {
        self.background_task.write().unwrap().cancel_token = Some(token);
    }

    pub fn cancel_background_task(&self) {
        let cancel_token = &self.background_task.read().unwrap().cancel_token;
        if let Some(cancel_token) = cancel_token {
            debug!("Cancelling background task");
            cancel_token.cancel();
        }
    }

    pub fn add_event_cancellation_token(&mut self, token: CancellationToken) {
        self.event_task.write().unwrap().cancel_token = Some(token);
    }

    fn flush_event_task(&self) -> Result<(), EventError> {
        Ok(self.event_tx.send(Event::Flush)?)
    }

    fn cancel_event_task(&self) {
        let cancel_token = &self.event_task.read().unwrap().cancel_token;
        if let Some(cancel_token) = cancel_token {
            debug!("Cancelling event task");
            cancel_token.cancel();
        }
    }

    pub fn add_event_abort_handle(&mut self, handle: JoinHandle<()>) {
        self.event_task
            .write()
            .unwrap()
            .abort_handle
            .replace(handle.abort_handle());
    }

    pub fn add_background_abort_handle(&mut self, handle: JoinHandle<()>) {
        self.background_task
            .write()
            .unwrap()
            .abort_handle
            .replace(handle.abort_handle());
    }

    pub fn is_event_running(&self) -> bool {
        self.event_task.read().unwrap().running
    }

    pub fn has_background_handle(&self) -> bool {
        self.background_task.read().unwrap().abort_handle.is_some()
    }

    pub fn is_background_running(&self) -> bool {
        self.background_task.read().unwrap().running
    }

    pub fn set_event_running(&self, running: bool) {
        let mut event_task = self.event_task.write().unwrap();
        event_task.running = running;
    }

    pub fn set_background_running(&self, running: bool) {
        let mut background_task = self.background_task.write().unwrap();
        background_task.running = running;
    }

    /// Aborts the background task.
    /// This will:
    ///     (1) Send the cancel signal to the background task via the CancellationToken
    ///     (2) Abort the background task's JoinHandle
    /// This is intended to be called when shutting down and after
    /// the associated queue has been flushed
    fn shutdown_background_task(&self) -> Result<(), EventError> {
        // check if handle
        self.cancel_background_task();

        // abort the background task
        let background_handle = {
            let guard = self.background_task.write().unwrap().abort_handle.take();
            guard
        };

        if let Some(handle) = background_handle {
            handle.abort();
            debug!("Background task handle shut down");
        }

        Ok(())
    }

    /// Aborts the background task.
    /// This will:
    ///     (1) Send the cancel signal to the event task via the CancellationToken
    ///     (2) Abort the event task's JoinHandle
    /// This is intended to be called when shutting down and after
    /// the associated queue has been flushed
    fn shutdown_event_task(&self) -> Result<(), EventError> {
        match self.flush_event_task() {
            Ok(_) => debug!("Event task flush signal sent"),
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("channel closed") {
                    debug!("Channel already closed for event task: {}", self.id);
                } else {
                    warn!("Failed to send flush signal to event task: {}", e);
                }
            }
        }

        debug!("Waiting 250 ms to allow time for flush before cancelling event task");
        std::thread::sleep(Duration::from_millis(250));

        self.cancel_event_task();

        // wait 250 ms to allow time for flush before aborting thread
        debug!("Waiting 250 ms to allow time for flush before aborting event task");
        std::thread::sleep(Duration::from_millis(250));

        // abort the event task
        let event_handle = {
            let guard = self.event_task.write().unwrap().abort_handle.take();
            guard
        };

        if let Some(handle) = event_handle {
            handle.abort();
            debug!("Event task handle shut down");
        }

        Ok(())
    }

    /// Shuts down all async tasks
    pub fn shutdown_tasks(&self) -> Result<(), EventError> {
        self.shutdown_background_task()?;
        self.shutdown_event_task()?;
        Ok(())
    }
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    pub task_state: TaskState,

    #[pyo3(get)]
    pub identifier: String,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new(task_state: TaskState, identifier: String) -> Self {
        debug!("Creating unbounded QueueBus for identifier: {}", identifier);

        Self {
            task_state,
            identifier,
        }
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        debug!(
            "Publishing event to QueueBus for identifier: {}",
            self.identifier
        );
        Ok(self.task_state.event_tx.send(event)?)
    }
}

#[pymethods]
impl QueueBus {
    /// Insert an event to the bus
    ///
    /// # Arguments
    /// * `event` - The event to publish
    pub fn insert(&self, item: &Bound<'_, PyAny>) -> Result<(), PyEventError> {
        let item = QueueItem::from_py_entity(item)
            .inspect_err(|e| error!("Failed to convert entity to QueueItem: {}", e))?;
        debug!(
            "Inserting event into QueueBus for identifier: {}: {:?}",
            self.identifier, item
        );
        let event = Event::Task(item);
        self.publish(event)?;
        Ok(())
    }
}
