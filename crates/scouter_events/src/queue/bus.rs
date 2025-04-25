use pyo3::prelude::*;
use scouter_error::{EventError, ScouterError};
use scouter_types::QueueEntity;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::{debug, instrument};

#[derive(Debug)]
pub enum Event {
    Task(QueueEntity),
}

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    tx: UnboundedSender<Event>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new() -> (Self, UnboundedReceiver<Event>, oneshot::Receiver<()>) {
        debug!("Creating unbounded QueueBus");
        let (tx, rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        (
            Self {
                tx,
                shutdown_tx: Some(shutdown_tx),
            },
            rx,
            shutdown_rx,
        )
    }

    #[instrument(skip_all)]
    pub fn publish(&self, event: Event) -> Result<(), EventError> {
        debug!("Publishing event: {:?}", event);
        self.tx
            .send(event)
            .map_err(|e| EventError::SendEntityError(e.to_string()))
    }

    #[instrument(skip_all)]
    pub fn shutdown_channel(&mut self) {
        debug!("Shutting down QueueBus");
        // Drop the sender which will close the channel
        self.tx = mpsc::unbounded_channel().0;

        // Signal shutdown
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

#[pymethods]
impl QueueBus {
    /// Insert an event to the bus
    ///
    /// # Arguments
    /// * `event` - The event to publish
    pub fn insert(&mut self, entity: &Bound<'_, PyAny>) -> Result<(), ScouterError> {
        let entity = QueueEntity::from_py_entity(entity)?;
        let event = Event::Task(entity);
        self.publish(event)?;
        Ok(())
    }

    /// Shutdown the bus
    /// This will send a messages to the background queue, which will trigger a flush on the queue
    pub fn shutdown(&mut self) {
        self.shutdown_channel();
    }
}
