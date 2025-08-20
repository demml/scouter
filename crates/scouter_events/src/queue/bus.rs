use crate::error::{EventError, PyEventError};
use pyo3::prelude::*;
use scouter_types::QueueItem;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::{debug, error, instrument};

#[derive(Debug)]
pub enum Event {
    Task(QueueItem),
    Init,
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    tx: UnboundedSender<Event>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    pub initialized: Arc<RwLock<bool>>,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new() -> (Self, UnboundedReceiver<Event>, oneshot::Receiver<()>) {
        debug!("Creating unbounded QueueBus");
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let initialized = Arc::new(RwLock::new(false));

        (
            Self {
                tx,
                shutdown_tx: Some(shutdown_tx),
                initialized,
            },
            rx,
            shutdown_rx,
        )
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        Ok(self.tx.send(event)?)
    }

    pub fn is_initialized(&self) -> bool {
        // Check if the bus is initialized
        if let Ok(initialized) = self.initialized.read() {
            *initialized
        } else {
            false
        }
    }
}

#[pymethods]
impl QueueBus {
    /// Insert an event to the bus
    ///
    /// # Arguments
    /// * `event` - The event to publish
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), PyEventError> {
        let entity = QueueItem::from_py_entity(entity)?;
        debug!("Inserting event into QueueBus: {:?}", entity);
        let event = Event::Task(entity);
        self.publish(event)?;
        Ok(())
    }

    /// Shutdown the bus
    /// This will send a messages to the background queue, which will trigger a flush on the queue
    #[instrument(skip_all)]
    pub fn shutdown(&mut self) {
        // Signal shutdown
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

impl QueueBus {
    /// Check if the bus is initialized
    #[instrument(skip_all, name = "queuebus_init")]
    pub fn init(&self, id: &str) -> Result<(), EventError> {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut attempts = 0;
        debug!("Initializing QueueBus with id: {:?}", id);

        while !self.is_initialized() {
            if attempts >= 100 {
                error!(
                    "Failed to initialize QueueBus after 100 attempts for id: {:?}",
                    id
                );
                return Err(EventError::InitializationError);
            }
            attempts += 1;
            std::thread::sleep(std::time::Duration::from_millis(100));

            let event = Event::Init;
            self.publish(event)?;
        }
        Ok(())
    }
}
