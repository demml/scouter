use std::sync::Arc;

use crate::{
    error::{EventError, PyEventError},
    queue::traits::queue::BackgroundEvent,
};
use pyo3::prelude::*;
use scouter_types::QueueItem;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument, warn};
#[derive(Debug)]
pub enum Event {
    Task(QueueItem),
}

#[derive(Debug)]
pub struct Loop {
    pub abort_handle: Option<AbortHandle>,
    pub loop_running: bool,
    pub cancel_token: Option<CancellationToken>,
}

#[derive(Debug, Clone)]
pub struct EventLoops {
    // track the loop that receives events
    pub event_loop: Arc<RwLock<Loop>>,

    // track the loop that processes background tasks (only applies to psi and custom)
    pub background_loop: Arc<RwLock<Loop>>,

    // channel to send events to the event loop
    pub event_tx: UnboundedSender<Event>,
}

impl EventLoops {
    pub fn cancel_background_task(&self) {
        let cancel_token = &self.background_loop.read().unwrap().cancel_token;
        if let Some(cancel_token) = cancel_token {
            cancel_token.cancel();
        }
    }

    pub fn cancel_event_task(&self) {
        let cancel_token = &self.event_loop.read().unwrap().cancel_token;
        if let Some(cancel_token) = cancel_token {
            cancel_token.cancel();
        }
    }

    pub fn add_event_abort_handle(&mut self, handle: JoinHandle<()>) {
        self.event_loop
            .write()
            .unwrap()
            .abort_handle
            .replace(handle.abort_handle());
    }

    pub fn add_background_abort_handle(&mut self, handle: JoinHandle<()>) {
        self.background_loop
            .write()
            .unwrap()
            .abort_handle
            .replace(handle.abort_handle());
    }

    pub fn is_event_loop_running(&self) -> bool {
        self.event_loop.read().unwrap().loop_running
    }

    pub fn has_background_handle(&self) -> bool {
        self.background_loop.read().unwrap().abort_handle.is_some()
    }

    pub fn is_background_loop_running(&self) -> bool {
        self.background_loop.read().unwrap().loop_running
    }

    pub fn set_event_loop_running(&self, running: bool) {
        let mut event_loop = self.event_loop.write().unwrap();
        event_loop.loop_running = running;
    }

    pub fn set_background_loop_running(&self, running: bool) {
        let mut background_loop = self.background_loop.write().unwrap();
        background_loop.loop_running = running;
    }

    /// Aborts the background loop.
    /// This will:
    ///     (1) Send the cancel signal to the background task via the CancellationToken
    ///     (2) Abort the background task's JoinHandle
    /// This is intended to be called when shutting down and after
    /// the associated queue has been flushed
    fn shutdown_background_task(&self) -> Result<(), EventError> {
        self.cancel_background_task();

        // abort the background loop
        let background_handle = {
            let guard = self.background_loop.write().unwrap().abort_handle.take();
            guard
        };

        if let Some(handle) = background_handle {
            handle.abort();
            debug!("Background loop handle aborted");
        }

        Ok(())
    }

    /// Aborts the background loop.
    /// This will:
    ///     (1) Send the cancel signal to the event task via the CancellationToken
    ///     (2) Abort the event task's JoinHandle
    /// This is intended to be called when shutting down and after
    /// the associated queue has been flushed
    fn shutdown_event_task(&self) -> Result<(), EventError> {
        self.cancel_event_task();

        // abort the event loop
        let event_handle = {
            let guard = self.event_loop.write().unwrap().abort_handle.take();
            guard
        };

        if let Some(handle) = event_handle {
            handle.abort();
            debug!("Event loop handle aborted");
        }

        Ok(())
    }

    /// Shuts down all async tasks
    pub fn shutdown_tasks(&self) -> Result<(), EventError> {
        self.shutdown_event_task()?;
        self.shutdown_background_task()?;
        Ok(())
    }
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    pub event_loops: EventLoops,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new(event_loops: EventLoops) -> Self {
        debug!("Creating unbounded QueueBus");

        Self { event_loops }
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        Ok(self.event_loops.event_tx.send(event)?)
    }
}

#[pymethods]
impl QueueBus {
    /// Insert an event to the bus
    ///
    /// # Arguments
    /// * `event` - The event to publish
    pub fn insert(&self, entity: &Bound<'_, PyAny>) -> Result<(), PyEventError> {
        let entity = QueueItem::from_py_entity(entity)?;
        debug!("Inserting event into QueueBus: {:?}", entity);
        let event = Event::Task(entity);
        self.publish(event)?;
        Ok(())
    }

    /// Shutdown the bus
    /// This will send a messages to the event and background queue, which will trigger a flush on the queue
    #[instrument(skip_all)]
    pub fn shutdown(&self) -> Result<(), PyEventError> {
        self.event_loops.shutdown_event_task()?;
        Ok(())
    }
}
