# Technical Component Specification: Scouter Queue

## Overview
The Scouter Queue is the primary interface for sending real-time data to the Scouter server from a python application. The Scouter Queue is built to be a lightweight, high-performance and transient interface for publishing data so that it doesn't get in the way of the application. To achieve this the Scouter Queue leverages a channel system to send and receive messages, which are then passed to the background producer independent of the running python application.


## Component Architecture

<img src="/scouter/docs/specs/assets/scouter-queue.png" alt="Scouter Queue Architecture" style="display: block; margin: 0 auto;" width="500"/>

## How it works

(1) **Scouter Queue**: The user creates a `ScouterQueue` by using the `from_path` operation which accepts a hashmap of paths to the queue files. The ScouterQueue will create a new queue for each path and start a background worker that will read from the queue and send the data to the Scouter server. The `from_path` operation also accepts a transport configuration that is used to setup the specific transport producer for the queue (kafka, rabbitmq, etc.).

```rust

#[pyclass]
pub struct ScouterQueue {
    queues: HashMap<String, Py<QueueBus>>,
    _shared_runtime: Arc<tokio::runtime::Runtime>,
    completion_rxs: HashMap<String, oneshot::Receiver<()>>,
    pub queue_state: Arc<HashMap<String, TaskState>>,
}

#[staticmethod]
    #[pyo3(signature = (path, transport_config))]
    pub fn from_path(
        py: Python,
        path: HashMap<String, PathBuf>,
        transport_config: &Bound<'_, PyAny>,
    ) -> Result<Self, EventError>
```

```python
class ScouterQueue:
    """Main queue class for Scouter. Publishes drift records to the configured transport"""

    @staticmethod
    def from_path(
        path: Dict[str, Path],
        transport_config: Union[KafkaConfig, RabbitMQConfig, HttpConfig],
    )
```

(2) For each `DriftProfile`, a spawned `event_handler` will be created that uses `Tokio::select` to keep track of received events (`Event enum`). Received events from the parent python thread are passed to the `event_handler`, which is then inserted into a `queue`. If the queue capacity has been reached, the events are published via the configured transport. In the case of `Psi` and `Custom` drift profiles, an additional `background_handler` is created that publishes events from the queue every 30 seconds. This is done in order to minimize any data loss if an app fails or to handle cases where an api may be receiving low amounts of traffic, which may cause the queue to have to wait awhile to fill up.

(3) For every `DriftProfile` a `TaskState` will be created that keeps track of the `event_handler` and `background_handler` tasks.The `TaskState` is used in shutdown functions to cancel spawned tasks via a `Tokio` `CancellationToken`.

The following is used to spawn the event handler:

```rust
#[allow(clippy::too_many_arguments)]
async fn spawn_queue_event_handler(
    mut event_rx: UnboundedReceiver<Event>,
    transport_config: TransportConfig,
    drift_profile: DriftProfile,
    runtime: Arc<runtime::Runtime>,
    id: String,
    mut task_state: TaskState,
    cancellation_token: CancellationToken,
) -> Result<(), EventError>
```

For `Psi` and `Custom` profiles, the background polling task is spawned as follows:

```rust
pub trait BackgroundTask: Send + Sync + 'static {
    type DataItem: QueueExt + Send + Sync + 'static;
    type Processor: FeatureQueue + Send + Sync + 'static;

    #[allow(clippy::too_many_arguments)]
    fn start_background_task(
        &self,
        data_queue: Arc<ArrayQueue<Self::DataItem>>,
        processor: Arc<Self::Processor>,
        mut producer: RustScouterProducer,
        last_publish: Arc<RwLock<DateTime<Utc>>>,
        runtime: Arc<Runtime>,
        queue_capacity: usize,
        identifier: String,
        task_state: TaskState,
        cancellation_token: CancellationToken,
    ) -> Result<JoinHandle<()>, EventError>
}
```


(4) **QueueBus**: Everything discussed so far has focused on the Rust background tasks that run independent of the python runtime. So how do we bridge the gap and get events to rust from python. For every `DriftProfile`, a `QueueBus` is created that exposes an `insert` method to the user. This method will accept any of the allowed data types for monitoring (`Features`, `Metrics`, `LLMRecord`). The data types are extracted and published as an `Event` enum to the event channel, which is then read by the event receiver (`tokio::sync:mpsc`) embedded within the rust `event_handler`. This publishing happends asynchronously, which allows the user on the python side to continue accepting api requests without impacting latency. On the rust side, the event receiver will then process the event and add it to the background queue.

```rust
#[pyclass(name = "Queue")]
pub struct QueueBus {
    pub task_state: TaskState,

    #[pyo3(get)]
    pub identifier: String,

}
```

(5) **Error Handling**: Errors are logged and not returned to the user. This is to ensure that the spawned tasks do not block the main thread and can continue to process events. As a user, it's important to monitor these logs.


(6) **Queue Insert**: After the `ScouterQueue` is created, the user can insert events into the queue by accessing the queue directly through its alias and calling the `insert` method. The insert method expects either a `Features` object, a `Metrics` object (for custom metrics) or an `LLMRecord` object (for llm as a judge workflows). Note - Scouter also provides a `FeatureMixin` class that can be used to convert a python object into a `Features` object. This is useful for converting a Pydantic BaseModel into a `Features` object. The `FeatureMixin` class is not required, but it is recommended for ease of use.

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
        PyHelperFuncs::__str__(self)
    }
}



#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct Metric {
    pub name: String,
    pub value: f64,
}

#[pymethods]
impl Metric {
    #[new]
    pub fn new(name: String, value: Bound<'_, PyAny>) -> Self {
        let value = if value.is_instance_of::<PyFloat>() {
            value.extract::<f64>().unwrap()
        } else if value.is_instance_of::<PyInt>() {
            value.extract::<i64>().unwrap() as f64
        } else {
            panic!(
                "Unsupported metric type: {}",
                value.get_type().name().unwrap()
            );
        };
        let lowercase_name = name.to_lowercase();
        Metric {
            name: lowercase_name,
            value,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
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


#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct LLMRecord {
    pub uid: String,

    pub space: String,

    pub name: String,

    pub version: String,

    pub created_at: DateTime<Utc>,

    pub context: Value,

    pub score: Value,

    pub prompt: Option<Value>,

    #[pyo3(get)]
    pub entity_type: EntityType,
}

#[pymethods]
impl LLMRecord {
    #[new]
    #[pyo3(signature = (
        context,
        prompt=None,
    ))]

    /// Creates a new LLMRecord instance.
    /// The context is either a python dictionary or a pydantic basemodel.
    pub fn new(
        py: Python<'_>,
        context: Bound<'_, PyAny>,
        prompt: Option<Bound<'_, PyAny>>,
    ) -> Result<Self, TypeError> {
        // check if context is a PyDict or PyObject(Pydantic model)
        let context_val = if context.is_instance_of::<PyDict>() {
            pyobject_to_json(&context)?
        } else if is_pydantic_basemodel(py, &context)? {
            // Dump pydantic model to dictionary
            let model = context.call_method0("model_dump")?;

            // Serialize the dictionary to JSON
            pyobject_to_json(&model)?
        } else {
            Err(TypeError::MustBeDictOrBaseModel)?
        };

        let prompt: Option<Value> = match prompt {
            Some(p) => {
                if p.is_instance_of::<Prompt>() {
                    let prompt = p.extract::<Prompt>()?;
                    Some(serde_json::to_value(prompt)?)
                } else {
                    Some(pyobject_to_json(&p)?)
                }
            }
            None => None,
        };

        Ok(LLMRecord {
            uid: create_uuid7(),
            created_at: Utc::now(),
            space: String::new(),
            name: String::new(),
            version: String::new(),
            context: context_val,
            score: Value::Null,
            prompt,
            entity_type: EntityType::LLM,
        })
    }
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

# or for custom metrics

queue["alias"].insert(
    Metrics(
        [
            Metric("mape", 1.0),
            Metric("mae", 2.0)
        ],
    )
)

# or for LLMRecords

queue["alias"].insert(
    LLMRecord(
        context={
            "input": bound_prompt.message[0].unwrap(),
            "response": response.result,
        },
    )
)
```

---

*Version: 1.0*
*Last Updated: 2025-08-25*
*Component Owner: Steven Forrester*