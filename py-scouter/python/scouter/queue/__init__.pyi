# pylint: skip-file

import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

from typing_extensions import Protocol, TypeAlias

from ..llm import Prompt
from ..mock import MockConfig
from ..observe import ObservabilityMetrics
from ..transport import HTTPConfig, KafkaConfig, RabbitMQConfig, RedisConfig

class EntityType:
    Feature = "EntityType"
    Metric = "EntityType"

class RecordType:
    Spc = "RecordType"
    Psi = "RecordType"
    Observability = "RecordType"
    Custom = "RecordType"

class ServerRecord:
    Spc: "ServerRecord"
    Psi: "ServerRecord"
    Custom: "ServerRecord"
    Observability: "ServerRecord"

    def __init__(self, record: Any) -> None:
        """Initialize server record

        Args:
            record:
                Server record to initialize
        """

    @property
    def record(
        self,
    ) -> Union[
        SpcServerRecord, PsiServerRecord, CustomMetricServerRecord, ObservabilityMetrics
    ]:
        """Return the drift server record."""

class ServerRecords:
    def __init__(self, records: List[ServerRecord]) -> None:
        """Initialize server records

        Args:
            records:
                List of server records
        """

    @property
    def records(self) -> List[ServerRecord]:
        """Return the drift server records."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

class SpcServerRecord:
    def __init__(
        self,
        space: str,
        name: str,
        version: str,
        feature: str,
        value: float,
    ):
        """Initialize spc drift server record

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            feature:
                Feature name
            value:
                Feature value
        """

    @property
    def created_at(self) -> datetime.datetime:
        """Return the created at timestamp."""

    @property
    def space(self) -> str:
        """Return the space."""

    @property
    def name(self) -> str:
        """Return the name."""

    @property
    def version(self) -> str:
        """Return the version."""

    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def value(self) -> float:
        """Return the sample value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class PsiServerRecord:
    def __init__(
        self,
        space: str,
        name: str,
        version: str,
        feature: str,
        bin_id: int,
        bin_count: int,
    ):
        """Initialize spc drift server record

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            feature:
                Feature name
            bin_id:
                Bundle ID
            bin_count:
                Bundle ID
        """

    @property
    def created_at(self) -> datetime.datetime:
        """Return the created at timestamp."""

    @property
    def space(self) -> str:
        """Return the space."""

    @property
    def name(self) -> str:
        """Return the name."""

    @property
    def version(self) -> str:
        """Return the version."""

    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def bin_id(self) -> int:
        """Return the bin id."""

    @property
    def bin_count(self) -> int:
        """Return the sample value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class CustomMetricServerRecord:
    def __init__(
        self,
        space: str,
        name: str,
        version: str,
        metric: str,
        value: float,
    ):
        """Initialize spc drift server record

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            metric:
                Metric name
            value:
                Metric value
        """

    @property
    def created_at(self) -> datetime.datetime:
        """Return the created at timestamp."""

    @property
    def space(self) -> str:
        """Return the space."""

    @property
    def name(self) -> str:
        """Return the name."""

    @property
    def version(self) -> str:
        """Return the version."""

    @property
    def metric(self) -> str:
        """Return the metric name."""

    @property
    def value(self) -> float:
        """Return the metric value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class Feature:
    def __init__(self, name: str, value: Any) -> None:
        """Initialize feature. Will attempt to convert the value to it's corresponding feature type.
        Current support types are int, float, string.

        Args:
            name:
                Name of the feature
            value:
                Value of the feature. Can be an int, float, or string.

        Example:
            ```python
            feature = Feature("feature_1", 1) # int feature
            feature = Feature("feature_2", 2.0) # float feature
            feature = Feature("feature_3", "value") # string feature
            ```
        """

    @staticmethod
    def int(name: str, value: int) -> "Feature":
        """Create an integer feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def float(name: str, value: float) -> "Feature":
        """Create a float feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def string(name: str, value: str) -> "Feature":
        """Create a string feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def categorical(name: str, value: str) -> "Feature":
        """Create a categorical feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

class Features:
    def __init__(
        self,
        features: List[Feature] | Dict[str, Union[int, float, str]],
    ) -> None:
        """Initialize a features class

        Args:
            features:
                List of features or a dictionary of key-value pairs.
                If a list, each item must be an instance of Feature.
                If a dictionary, each key is the feature name and each value is the feature value.
                Supported types for values are int, float, and string.

        Example:
            ```python
            # Passing a list of features
            features = Features(
                features=[
                    Feature.int("feature_1", 1),
                    Feature.float("feature_2", 2.0),
                    Feature.string("feature_3", "value"),
                ]
            )

            # Passing a dictionary (pydantic model) of features
            class MyFeatures(BaseModel):
                feature1: int
                feature2: float
                feature3: str

            my_features = MyFeatures(
                feature1=1,
                feature2=2.0,
                feature3="value",
            )

            features = Features(my_features.model_dump())
            ```
        """

    def __str__(self) -> str:
        """Return the string representation of the features"""

    @property
    def features(self) -> List[Feature]:
        """Return the list of features"""

    @property
    def entity_type(self) -> EntityType:
        """Return the entity type"""

class Metric:
    def __init__(self, name: str, value: float | int) -> None:
        """Initialize metric

        Args:
            name:
                Name of the metric
            value:
                Value to assign to the metric. Can be an int or float but will be converted to float.
        """

    def __str__(self) -> str:
        """Return the string representation of the metric"""

    @property
    def metrics(self) -> List[Metric]:
        """Return the list of metrics"""

    @property
    def entity_type(self) -> EntityType:
        """Return the entity type"""

class Metrics:
    def __init__(self, metrics: List[Metric] | Dict[str, Union[int, float]]) -> None:
        """Initialize metrics

        Args:
            metrics:
                List of metrics or a dictionary of key-value pairs.
                If a list, each item must be an instance of Metric.
                If a dictionary, each key is the metric name and each value is the metric value.


        Example:
            ```python

            # Passing a list of metrics
            metrics = Metrics(
                metrics=[
                    Metric("metric_1", 1.0),
                    Metric("metric_2", 2.5),
                    Metric("metric_3", 3),
                ]
            )

            # Passing a dictionary (pydantic model) of metrics
            class MyMetrics(BaseModel):
                metric1: float
                metric2: int

            my_metrics = MyMetrics(
                metric1=1.0,
                metric2=2,
            )

            metrics = Metrics(my_metrics.model_dump())
        """

    def __str__(self) -> str:
        """Return the string representation of the metrics"""

class Queue:
    """Individual queue associated with a drift profile"""

    def insert(self, entity: Union[Features, Metrics, LLMRecord]) -> None:
        """Insert a record into the queue

        Args:
            entity:
                Entity to insert into the queue.
                Can be an instance for Features, Metrics, or LLMRecord.

        Example:
            ```python
            features = Features(
                features=[
                    Feature("feature_1", 1),
                    Feature("feature_2", 2.0),
                    Feature("feature_3", "value"),
                ]
            )
            queue.insert(features)
            ```
        """

    @property
    def identifier(self) -> str:
        """Return the identifier of the queue"""

class ScouterQueue:
    """Main queue class for Scouter. Publishes drift records to the configured transport"""

    @staticmethod
    def from_path(
        path: Dict[str, Path],
        transport_config: Union[
            KafkaConfig,
            RabbitMQConfig,
            RedisConfig,
            HTTPConfig,
        ],
    ) -> ScouterQueue:
        """Initializes Scouter queue from one or more drift profile paths

        Args:
            path (Dict[str, Path]):
                Dictionary of drift profile paths.
                Each key is a user-defined alias for accessing a queue
            transport_config (Union[KafkaConfig, RabbitMQConfig, RedisConfig, HTTPConfig]):
                Transport configuration for the queue publisher
                Can be KafkaConfig, RabbitMQConfig RedisConfig, or HTTPConfig

        Example:
            ```python
            queue = ScouterQueue(
                path={
                    "spc": Path("spc_profile.json"),
                    "psi": Path("psi_profile.json"),
                },
                transport_config=KafkaConfig(
                    brokers="localhost:9092",
                    topic="scouter_topic",
                ),
            )

            queue["psi"].insert(
                Features(
                    features=[
                        Feature("feature_1", 1),
                        Feature("feature_2", 2.0),
                        Feature("feature_3", "value"),
                    ]
                )
            )
            ```
        """

    def __getitem__(self, key: str) -> Queue:
        """Get the queue for the specified key

        Args:
            key (str):
                Key to get the queue for

        """

    def shutdown(self) -> None:
        """Shutdown the queue. This will close and flush all queues and transports"""

    @property
    def transport_config(
        self,
    ) -> Union[KafkaConfig, RabbitMQConfig, RedisConfig, HTTPConfig, MockConfig]:
        """Return the transport configuration used by the queue"""

class BaseModel(Protocol):
    """Protocol for pydantic BaseModel to ensure compatibility with context"""

    def model_dump(self) -> Dict[str, Any]:
        """Dump the model as a dictionary"""
        ...

    def model_dump_json(self) -> str:
        """Dump the model as a JSON string"""
        ...

    def __str__(self) -> str:
        """String representation of the model"""
        ...

SerializedType: TypeAlias = Union[str, int, float, dict, list]
Context: TypeAlias = Union[Dict[str, Any], BaseModel]

class LLMRecord:
    """LLM record containing context tied to a Large Language Model interaction
    that is used to evaluate drift in LLM responses.


    Examples:
        >>> record = LLMRecord(
        ...     context={
        ...         "input": "What is the capital of France?",
        ...         "response": "Paris is the capital of France."
        ...     },
        ... )
        >>> print(record.context["input"])
        "What is the capital of France?"
    """

    prompt: Optional[Prompt]
    """Optional prompt configuration associated with this record."""

    entity_type: EntityType
    """Type of entity, always EntityType.LLM for LLMRecord instances."""

    def __init__(
        self,
        context: Context,
        prompt: Optional[Prompt | SerializedType] = None,
    ) -> None:
        """Creates a new LLM record to associate with an `LLMDriftProfile`.
        The record is sent to the `Scouter` server via the `ScouterQueue` and is
        then used to inject context into the evaluation prompts.

        Args:
            context:
                Additional context information as a dictionary or a pydantic BaseModel. During evaluation,
                this will be merged with the input and response data and passed to the assigned
                evaluation prompts. So if you're evaluation prompts expect additional context via
                bound variables (e.g., `${foo}`), you can pass that here as key value pairs.
                {"foo": "bar"}
            prompt:
                Optional prompt configuration associated with this record. Can be a Potatohead Prompt or
                a JSON-serializable type.

        Raises:
            TypeError: If context is not a dict or a pydantic BaseModel.

        """
        ...

    @property
    def context(self) -> Dict[str, Any]:
        """Get the contextual information.

        Returns:
            The context data as a Python object (deserialized from JSON).

        Raises:
            TypeError: If the stored JSON cannot be converted to a Python object.
        """
        ...
