use crate::error::{EventError, PyEventError};
use pyo3::prelude::*;
use scouter_types::QueueItem;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::{debug, instrument};

#[derive(Debug)]
pub enum Event {
    Task(QueueItem),
    Stop,
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    tx: UnboundedSender<Event>,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new() -> (Self, UnboundedReceiver<Event>) {
        debug!("Creating unbounded QueueBus");
        let (tx, rx) = mpsc::unbounded_channel();

        (Self { tx }, rx)
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        Ok(self.tx.send(event)?)
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
    pub fn shutdown(&mut self) -> Result<(), PyEventError> {
        // Signal shutdown
        let event = Event::Stop;
        self.publish(event)?;
        Ok(())
    }
}
