use std::sync::Arc;

use crate::error::{EventError, PyEventError};
use crate::queue::traits::queue::BackgroundEvent;
use pyo3::prelude::*;
use scouter_types::QueueItem;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub enum Event {
    Start,
    Task(QueueItem),
    Stop,
}

#[derive(Debug, Clone)]
pub struct EventLoops {
    // track the loop that receives events
    pub event_loop: Arc<RwLock<Option<JoinHandle<()>>>>,
    pub event_loop_running: Arc<RwLock<bool>>,
    pub event_tx: UnboundedSender<Event>,

    // track the loop that processes background tasks (only applies to psi and custom)
    pub background_loop: Arc<RwLock<Option<JoinHandle<()>>>>,
    pub background_loop_running: Arc<RwLock<bool>>,
    pub background_tx: UnboundedSender<BackgroundEvent>,
}

impl EventLoops {
    pub fn is_event_loop_running(&self) -> bool {
        *self.event_loop_running.read().unwrap()
    }

    pub fn is_background_loop_running(&self) -> bool {
        *self.background_loop_running.read().unwrap()
    }

    pub fn running(&self) -> bool {
        let event_running = self.is_event_loop_running();

        // if background loop has some, check if running, if no handle, default to true
        let background_running = if self.background_loop.read().unwrap().is_some() {
            self.is_background_loop_running()
        } else {
            true
        };
        event_running && background_running
    }

    pub fn shutdown_event_loops(&self) {
        let mut event_loop_running = self.event_loop_running.write().unwrap();
        *event_loop_running = false;
    }

    pub fn start_event_loops(&self) {
        let mut event_loop_running = self.event_loop_running.write().unwrap();
        *event_loop_running = true;
    }

    pub fn shutdown(&self) -> Result<(), EventError> {
        self.event_tx.send(Event::Stop)?;
        Ok(())
    }
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    pub event_loops: Arc<EventLoops>,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new() -> (
        Self,
        UnboundedReceiver<Event>,
        UnboundedReceiver<BackgroundEvent>,
    ) {
        debug!("Creating unbounded QueueBus");
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (background_tx, background_rx) = mpsc::unbounded_channel();

        let event_loops = Arc::new(EventLoops {
            event_loop: Arc::new(RwLock::new(None)),
            event_loop_running: Arc::new(RwLock::new(false)),
            event_tx,
            background_loop: Arc::new(RwLock::new(None)),
            background_loop_running: Arc::new(RwLock::new(false)),
            background_tx,
        });

        (Self { event_loops }, event_rx, background_rx)
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
    /// This will send a messages to the background queue, which will trigger a flush on the queue
    #[instrument(skip_all)]
    pub fn shutdown(&self) -> Result<(), PyEventError> {
        // Signal shutdown
        let event = Event::Stop;
        self.publish(event)?;
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn start(&self) -> Result<(), PyEventError> {
        // Signal start
        let event = Event::Start;
        self.publish(event)?;
        Ok(())
    }
}

impl QueueBus {
    pub fn confirm_start(&self) -> Result<(), EventError> {
        // Signal confirm start
        let mut max_retries = 20;
        while max_retries > 0 {
            if self.event_loops.is_event_loop_running() {
                debug!("Event loop started successfully");
                return Ok(());
            }
            max_retries -= 1;
            std::thread::sleep(Duration::from_millis(100));
        }
        error!("Event loop failed to start");
        Err(EventError::EventLoopFailedToStartError)
    }
}
