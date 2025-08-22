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

#[derive(Debug)]
pub struct EventLoop {
    pub event_loop: Option<JoinHandle<()>>,
    pub event_loop_running: bool,
    pub event_tx: UnboundedSender<Event>,
}

#[derive(Debug)]
pub struct BackgroundLoop {
    pub background_loop: Option<JoinHandle<()>>,
    pub background_loop_running: bool,
    pub background_tx: UnboundedSender<BackgroundEvent>,
}

#[derive(Debug, Clone)]
pub struct EventLoops {
    // track the loop that receives events
    pub event_loop: Arc<RwLock<EventLoop>>,

    // track the loop that processes background tasks (only applies to psi and custom)
    pub background_loop: Arc<RwLock<BackgroundLoop>>,
}

impl EventLoops {
    pub fn add_event_handle(&mut self, handle: JoinHandle<()>) {
        self.event_loop.write().unwrap().event_loop.replace(handle);
    }

    pub fn add_background_handle(&mut self, handle: JoinHandle<()>) {
        self.background_loop
            .write()
            .unwrap()
            .background_loop
            .replace(handle);
    }
    pub fn is_event_loop_running(&self) -> bool {
        self.event_loop.read().unwrap().event_loop_running
    }

    pub fn is_background_loop_running(&self) -> bool {
        self.background_loop.read().unwrap().background_loop_running
    }

    pub fn running(&self) -> bool {
        let event_running = self.is_event_loop_running();

        // if background loop has some, check if running, if no handle, default to true
        let background_running = if self
            .background_loop
            .read()
            .unwrap()
            .background_loop
            .is_some()
        {
            self.is_background_loop_running()
        } else {
            true
        };
        event_running && background_running
    }

    pub fn set_event_loop_running(&self, running: bool) {
        let mut event_loop = self.event_loop.write().unwrap();
        event_loop.event_loop_running = running;
    }

    pub fn set_background_loop_running(&self, running: bool) {
        let mut background_loop = self.background_loop.write().unwrap();
        background_loop.background_loop_running = running;
    }

    fn abort_background_loop(&self) -> Result<(), EventError> {
        let background_handle = {
            let guard = self.background_loop.write().unwrap().background_loop.take();
            guard
        };

        if let Some(handle) = background_handle {
            handle.abort();
            debug!("Background loop handle aborted");
        }

        Ok(())
    }

    fn abort_event_loop(&self) -> Result<(), EventError> {
        let event_handle = {
            let guard = self.event_loop.write().unwrap().event_loop.take();
            guard
        };

        if let Some(handle) = event_handle {
            handle.abort();
            debug!("Event loop handle aborted");
        }

        Ok(())
    }

    pub async fn shutdown_background_task(&self) -> Result<(), EventError> {
        // signal should have already been sent. wait for the background task to finish
        let mut max_retries = 50;
        while self.background_loop.read().unwrap().background_loop_running {
            std::thread::sleep(Duration::from_millis(100));
            max_retries -= 1;
            if max_retries == 0 {
                error!("Timed out waiting for background loop to stop. Aborting the thread");
                self.abort_background_loop()?;
                return Ok(());
            }
        }

        let background_handle = {
            let guard = self.background_loop.write().unwrap().background_loop.take();
            guard
        };

        if let Some(handle) = background_handle {
            handle.await?;
            debug!("Background loop handle awaited");
        }

        // await background task completion
        Ok(())
    }

    pub fn wait_for_background_task_to_stop(&self) -> Result<(), EventError> {
        debug!("Waiting for background task to stop");
        //if background task is some wait for the
        if self
            .background_loop
            .read()
            .unwrap()
            .background_loop
            .is_some()
        {
            let mut max_retries = 50;
            while self.background_loop.read().unwrap().background_loop_running {
                std::thread::sleep(Duration::from_millis(100));
                max_retries -= 1;
                if max_retries == 0 {
                    error!("Timed out waiting for background loop to stop. Aborting the thread");
                    self.abort_background_loop()?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Shutdown the event task
    /// This needs to be sync to work with exposed python func, so choosing to abort the thread here
    /// vs await
    #[instrument(skip_all)]
    pub fn shutdown_event_task(&self) -> Result<(), EventError> {
        // send stop signal to event loop - this will also trigger a background task shutdown if present
        self.event_loop.read().unwrap().event_tx.send(Event::Stop)?;

        // Background should be existed before event
        self.wait_for_background_task_to_stop()?;

        let mut max_retries = 50;
        while self.event_loop.read().unwrap().event_loop_running {
            std::thread::sleep(Duration::from_millis(100));
            max_retries -= 1;
            if max_retries == 0 {
                error!("Timed out waiting for event loop to stop. Aborting the thread");
                self.abort_event_loop()?;
                return Ok(());
            }
        }

        self.abort_event_loop()?;

        // await background task completion
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
    pub fn new() -> (
        Self,
        UnboundedReceiver<Event>,
        UnboundedReceiver<BackgroundEvent>,
    ) {
        debug!("Creating unbounded QueueBus");
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (background_tx, background_rx) = mpsc::unbounded_channel();

        // get background loop
        let background_loop = Arc::new(RwLock::new(BackgroundLoop {
            background_loop: None,
            background_loop_running: false,
            background_tx,
        }));

        // get event loop
        let event_loop = Arc::new(RwLock::new(EventLoop {
            event_loop: None,
            event_loop_running: false,
            event_tx,
        }));

        let event_loops = EventLoops {
            event_loop,
            background_loop,
        };

        (Self { event_loops }, event_rx, background_rx)
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        Ok(self
            .event_loops
            .event_loop
            .read()
            .unwrap()
            .event_tx
            .send(event)?)
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
        self.publish(Event::Stop)?;
        Ok(())
    }

    #[instrument(skip_all)]
    pub fn start(&self) -> Result<(), PyEventError> {
        self.publish(Event::Start)?;
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
