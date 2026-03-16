use std::sync::Arc;

use crate::error::{EventError, PyEventError};
use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context as OtelContext;
use pyo3::prelude::*;
use scouter_types::{EvalRecord, QueueItem, TraceId};
use std::sync::RwLock;
use tokio::sync::Notify;
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
    pub startup_notify: Arc<Notify>,
}

impl Task {
    pub fn new() -> Self {
        Self {
            abort_handle: None,
            running: false,
            cancel_token: None,
            startup_notify: Arc::new(Notify::new()),
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
    pub fn notify_event_started(&self) {
        self.event_task.read().unwrap().startup_notify.notify_one();
    }

    pub fn notify_background_started(&self) {
        self.background_task
            .read()
            .unwrap()
            .startup_notify
            .notify_one();
    }

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

type PyQueueItem = Py<PyAny>;

/// QueueBus is an mpsc bus that allows for publishing events to subscribers.
/// It leverage an unbounded channel
/// Primary way to publish non-blocking events to background queues with ScouterQueue
#[pyclass(name = "Queue")]
pub struct QueueBus {
    pub task_state: TaskState,

    #[pyo3(get)]
    pub identifier: String,

    // for tracing purposes
    #[pyo3(get)]
    pub entity_uid: String,

    /// Capture store: `None` = disabled (default), `Some(Vec)` = enabled.
    record_store: Arc<RwLock<Option<Vec<PyQueueItem>>>>,
}

/// If `record` has no `trace_id` and there is an active OTel span, stamps the
/// record's `trace_id` from the current span context and returns the stamped
/// `TraceId`. Returns `None` when no stamping occurred (either the record
/// already had a `trace_id`, or there is no valid active span).
fn stamp_otel_trace_id(record: &mut EvalRecord) -> Option<TraceId> {
    if record.trace_id.is_some() {
        return None;
    }
    let cx = OtelContext::current();
    let span_ctx = cx.span().span_context().clone();
    if span_ctx.is_valid() {
        let trace_id = TraceId::from_bytes(span_ctx.trace_id().to_bytes());
        record.trace_id = Some(trace_id);
        Some(trace_id)
    } else {
        None
    }
}

const SCENARIO_TAG_BAGGAGE_KEY: &str = "scouter.eval.scenario_id";

/// If there is a `scouter.eval.scenario_id` entry in the current OTel baggage and the
/// record does not already carry that tag, appends `"scouter.eval.scenario_id=<value>"`
/// to `record.tags` and returns the formatted tag string.
fn stamp_scenario_tag(record: &mut EvalRecord) -> Option<String> {
    let cx = OtelContext::current();
    for (key, (value, _)) in cx.baggage() {
        if key.as_str() == SCENARIO_TAG_BAGGAGE_KEY {
            let tag = format!("{}={}", SCENARIO_TAG_BAGGAGE_KEY, value);
            if !record.tags.contains(&tag) {
                record.tags.push(tag.clone());
            }
            return Some(tag);
        }
    }
    None
}

impl QueueBus {
    #[instrument(skip_all)]
    pub fn new(task_state: TaskState, identifier: String, entity_uid: String) -> Self {
        debug!("Creating unbounded QueueBus for identifier: {}", identifier);

        Self {
            task_state,
            identifier,
            entity_uid,
            record_store: Arc::new(RwLock::new(None)),
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
        let mut extracted_item = QueueItem::from_py_entity(item)
            .inspect_err(|e| error!("Failed to convert entity to QueueItem: {}", e))?;
        debug!(
            "Inserting event into QueueBus for identifier: {}: {:?}",
            self.identifier, extracted_item
        );

        if let QueueItem::GenAI(ref mut record) = extracted_item {
            if let Some(trace_id) = stamp_otel_trace_id(record) {
                if let Ok(py_record) = item.cast::<EvalRecord>() {
                    py_record.borrow_mut().trace_id = Some(trace_id);
                } else {
                    warn!("stamp_otel_trace_id: could not cast Python item to EvalRecord; Python-side trace_id not updated");
                }
            }
            // Auto-stamp scenario tag from OTel baggage
            if let Some(tag) = stamp_scenario_tag(record) {
                if let Ok(py_record) = item.cast::<EvalRecord>() {
                    let mut borrowed = py_record.borrow_mut();
                    if !borrowed.tags.contains(&tag) {
                        borrowed.tags.push(tag);
                    }
                }
            }
        }

        {
            let mut store = self.record_store.write().unwrap();
            if let Some(store) = store.as_mut() {
                if matches!(extracted_item, QueueItem::GenAI(_)) {
                    store.push(item.clone().unbind());
                }
            }
        }

        self.publish(Event::Task(extracted_item))?;
        Ok(())
    }

    /// Enable in-process record capture for offline development.
    ///
    /// Once enabled, every `EvalRecord` inserted via `insert()` is also stored
    /// in memory and can be retrieved with `drain()`. Has no effect if capture
    /// is already enabled.
    pub fn enable_capture(&self) {
        let mut guard = self.record_store.write().unwrap();
        if guard.is_none() {
            *guard = Some(Vec::new());
        }
    }

    /// Disable record capture and discard any buffered records.
    pub fn disable_capture(&self) {
        let mut guard = self.record_store.write().unwrap();
        *guard = None;
    }

    /// Drain and return all captured `EvalRecord`s, clearing the internal buffer.
    ///
    /// Returns an empty list when capture is disabled.
    pub fn drain(&self) -> PyResult<Vec<PyQueueItem>> {
        let records: Vec<PyQueueItem> = {
            let mut guard = self.record_store.write().unwrap();
            if let Some(store) = guard.as_mut() {
                std::mem::take(store)
            } else {
                Vec::new()
            }
        };

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{Tracer as OTelTracer, TracerProvider};
    use opentelemetry_sdk::trace::SdkTracerProvider;

    #[test]
    fn test_stamp_otel_trace_id_with_active_span() {
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test");

        tracer.in_span("test_span", |_cx| {
            let mut record = EvalRecord::default();
            assert!(record.trace_id.is_none());

            let result = stamp_otel_trace_id(&mut record);

            assert!(result.is_some(), "expected trace_id to be stamped");
            assert!(record.trace_id.is_some(), "record.trace_id should be set");
        });
    }

    #[test]
    fn test_stamp_otel_trace_id_without_active_span() {
        let mut record = EvalRecord::default();
        let result = stamp_otel_trace_id(&mut record);

        assert!(
            result.is_none(),
            "no active span — nothing should be stamped"
        );
        assert!(record.trace_id.is_none());
    }

    #[test]
    fn test_stamp_otel_trace_id_not_overwritten_when_present() {
        let existing = TraceId::from_bytes([42u8; 16]);
        let mut record = EvalRecord {
            trace_id: Some(existing),
            ..Default::default()
        };

        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test");

        tracer.in_span("test_span", |_cx| {
            let result = stamp_otel_trace_id(&mut record);
            assert!(
                result.is_none(),
                "existing trace_id must not be overwritten"
            );
            assert_eq!(record.trace_id, Some(existing));
        });
    }

    #[test]
    fn test_stamp_otel_trace_id_consistent_within_span() {
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("test");

        tracer.in_span("test_span", |_cx| {
            let mut r1 = EvalRecord::default();
            let mut r2 = EvalRecord::default();

            stamp_otel_trace_id(&mut r1);
            stamp_otel_trace_id(&mut r2);

            assert_eq!(
                r1.trace_id, r2.trace_id,
                "both records should carry the same trace_id within one span"
            );
        });
    }
}
