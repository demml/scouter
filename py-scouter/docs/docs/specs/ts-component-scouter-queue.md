# Technical Component Specification: Scouter Queue

## Overview
The Scouter Queue is the primary interface for sending real-time data to the Scouter server from a python application. The Scouter Queue is built to be a lightweight, high-performance and transient interface for publishing data so that it doesn't get in the way of the application. To achieve this the Scouter Queue leverages a channel system to send and receive messages, which are then passed to the background producer independent on the running python application.


## Component Architecture

<img src="../assets/scouter-queue.png" alt="Scouter Queue Architecture" style="display: block; margin: 0 auto;" width="500"/>

## How it works

(1) **Scouter Queue**: The user creates a Scouter Queue by using the `from_path` operation which accepts a hashmap of paths to the queue files. The Scouter Queue will create a new queue for each path and start a background worker that will read from the queue and send the data to the Scouter server. The `from_path` operation also accepts a transport configuration that is used to setup the specific transport producer for the queue (kafka, rabbitmq, etc.).

```rust

#[pyclass]
pub struct ScouterQueue {
    queues: HashMap<String, Py<QueueBus>>,
    _shared_runtime: Arc<tokio::runtime::Runtime>,
    completion_rxs: HashMap<String, oneshot::Receiver<()>>,
}

#[staticmethod]
    #[pyo3(signature = (path, transport_config))]
    pub fn from_path(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
    ) -> Result<Self, ScouterError>
```

```python
class ScouterQueue:
    """Main queue class for Scouter. Publishes drift records to the configured transport"""

    @staticmethod
    def from_path(
        path: Dict[str, Path],
        transport_config: Union[KafkaConfig, RabbitMQConfig, HTTPConfig],
    )
```

(2) **QueueBus**: The QueueBus is the primary channeling system for sending messages from the Scouter Queue to the background producer worker. For each profile, a QueueBus is created with an unbounded sender (tx) and receiver (rx) along with a shutdown sender and receiver. Tx and Rx are built via `tokio::sync:mpsc` and the shutdown channel is built via `tokio::sync::oneshot`. The QueueBus is responsible for holding the tx and shutdown tx senders, while passing the rx and shutdown rx receivers to the background worker.

```rust
#[pyclass(name = "Queue")]
pub struct QueueBus {
    tx: UnboundedSender<Event>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}
```

(3) **Background Queue**: The background queue is started within a spawned runtime via a `handle_queue_events` async function. The background queue will create a new producer based on the provided TransportConfig and drift profile. It will then start a continuous loop using a tokio::select! macro to listen for events from the queue and shutdown signals. The background queue will also handle any errors that occur during the processing of events and will log them to the console. Note - errors are logged and not returned to the user. This is to ensure that the background queue does not block the main thread and can continue to process events.

```rust
#[allow(clippy::too_many_arguments)]
async fn handle_queue_events(
    mut rx: UnboundedReceiver<Event>,
    mut shutdown_rx: oneshot::Receiver<()>,
    drift_profile: DriftProfile,
    config: TransportConfig,
    id: String,
    queue_runtime: Arc<tokio::runtime::Runtime>,
    startup_tx: oneshot::Sender<()>,
    completion_tx: oneshot::Sender<()>,
) -> Result<(), EventError>
```

(4) **Queue Insert**: After the `ScouterQueue` is created, the user can insert events into the queue by accessing the queue directly through its alias and calling the `insert` method. The insert method expects either a `Features` object or a `Metrics` object (for custom metrics). Note - Scouter also provided a `FeatureMixin` class that can be used to convert a python object into a `Features` object. This is useful for converting a Pydantic BaseModel into a `Features` object. The `FeatureMixin` class is not required, but it is recommended for ease of use.

```rust
#[pyclass]
#[derive(Clone, Debug, Serialize)]
pub struct Features {
    #[pyo3(get)]
    pub features: Vec<Feature>,

    #[pyo3(get)]
    pub entity_type: EntityType,
}


#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Feature {
    Int(IntFeature),
    Float(FloatFeature),
    String(StringFeature),
}

#[pymethods]
impl Feature {
    #[staticmethod]
    pub fn int(name: String, value: i64) -> Self {
        Feature::Int(IntFeature { name, value })
    }

    #[staticmethod]
    pub fn float(name: String, value: f64) -> Self {
        Feature::Float(FloatFeature { name, value })
    }

    #[staticmethod]
    pub fn string(name: String, value: String) -> Self {
        Feature::String(StringFeature { name, value })
    }

    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
    }
}


#[pymethods]
impl Metric {
    #[new]
    pub fn new(name: String, value: f64) -> Self {
        Metric { name, value }
    }
    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct Metrics {
    #[pyo3(get)]
    pub metrics: Vec<Metric>,

    #[pyo3(get)]
    pub entity_type: EntityType,
}

```

### Python example
```python
class PredictRequest(BaseModel):
    feature_1: float
    feature_2: float
    feature_3: float

    def to_features(self) -> Features:
        return Features(
            features=[
                Feature.float("feature_1", self.feature_1),
                Feature.float("feature_2", self.feature_2),
                Feature.float("feature_3", self.feature_3),
            ]
        )

queue = ScouterQueue.from_path(...)
queue["alias"].insert(request.to_features())

# or

queue["alias"].insert(
    Metrics(
        [
            Metric("mape", 1.0), 
            Metric("mae", 2.0)
        ],
    )
)
```

---

*Version: 1.0*  
*Last Updated: 2025-04-29*  
*Component Owner: Steven Forrester*