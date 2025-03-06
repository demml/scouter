# pylint: skip-file

import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

from ..client import GetProfileRequest, HTTPConfig
from ..drift import CustomDriftProfile, PsiDriftProfile, SpcDriftProfile
from ..logging import LogLevel
from ..observe import ObservabilityMetrics

class RecordType:
    Spc = "RecordType"
    Psi = "RecordType"
    Observability = "RecordType"
    Custom = "RecordType"

class KafkaConfig:
    brokers: str
    topic: str
    compression_type: str
    message_timeout_ms: int
    message_max_bytes: int
    log_level: LogLevel
    config: Dict[str, str]
    max_retries: int

    def __init__(
        self,
        brokers: Optional[str] = None,
        topic: Optional[str] = None,
        compression_type: Optional[str] = None,
        raise_on_error: bool = False,
        message_timeout_ms: int = 600_000,
        message_max_bytes: int = 2097164,
        log_level: LogLevel = LogLevel.Info,
        config: Dict[str, str] = {},
        max_retries: int = 3,
    ) -> None:
        """Kafka configuration to use with the KafkaProducer.

        Args:
            brokers:
                Comma-separated list of Kafka brokers.
                If not provided, the value of the KAFKA_BROKERS environment variable is used.

            topic:
                Kafka topic to publish messages to.
                If not provided, the value of the KAFKA_TOPIC environment variable is used.

            compression_type:
                Compression type to use for messages.
                Default is "gzip".

            raise_on_error:
                Whether to raise an error if message delivery fails.
                Default is True.

            message_timeout_ms:
                Message timeout in milliseconds.
                Default is 600_000.

            message_max_bytes:
                Maximum message size in bytes.
                Default is 2097164.

            log_level:
                Log level for the Kafka producer.
                Default is LogLevel.Info.

            config:
                Additional Kafka configuration options. These will be passed to the Kafka producer.
                See https://kafka.apache.org/documentation/#configuration.

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.

        """

class RabbitMQConfig:
    address: str
    queue: str
    raise_on_error: bool
    max_retries: int

    def __init__(
        self,
        host: Optional[str] = None,
        port: Optional[int] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        queue: Optional[str] = None,
        raise_on_error: bool = False,
        max_retries: int = 3,
    ) -> None:
        """RabbitMQ configuration to use with the RabbitMQProducer.

        Args:
            host:
                RabbitMQ host.
                If not provided, the value of the RABBITMQ_HOST environment variable is used.

            port:
                RabbitMQ port.
                If not provided, the value of the RABBITMQ_PORT environment variable is used.

            username:
                RabbitMQ username.
                If not provided, the value of the RABBITMQ_USERNAME environment variable is used.

            password:
                RabbitMQ password.
                If not provided, the value of the RABBITMQ_PASSWORD environment variable is used.

            queue:
                RabbitMQ queue to publish messages to.
                If not provided, the value of the RABBITMQ_QUEUE environment variable is used.

            raise_on_error:
                Whether to raise an error if message delivery fails.
                Default is False.

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.
        """

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
    def record(self) -> Union[SpcServerRecord, PsiServerRecord, CustomMetricServerRecord, ObservabilityMetrics]:
        """Return the drift server record."""

class ServerRecords:
    def __init__(self, records: List[ServerRecord], record_type: RecordType) -> None:
        """Initialize server records

        Args:
            records:
                List of server records
            record_type:
                Type of server records
        """

    @property
    def record_type(self) -> RecordType:
        """Return the drift type."""

    @property
    def records(self) -> List[ServerRecord]:
        """Return the drift server records."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

class ScouterProducer:
    def __init__(
        self,
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ) -> None:
        """Top-level Producer class.

        Args:
            config:
                Configuration object for the producer that specifies the type of producer to use.

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.
        """

    def publish(self, message: ServerRecords) -> None:
        """Publish a message to the queue.

        Args:
            message:
                Message to publish.
        """

    def flush(self) -> None:
        """Flush the producer queue."""

class DriftTransportConfig:
    def __init__(
        self,
        id: str,
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
        drift_profile_path: Optional[Path] = None,
        drift_profile_request: Optional[GetProfileRequest] = None,
        scouter_server_config: Optional[HTTPConfig] = None,
    ) -> None:
        """Drift transport configuration. To be used with ScouterQueue.

        Args:
            id:
                Unique identifier for the drift transport configuration.
            config:
                Configuration object for the producer that specifies the type of producer to use.

            drift_profile_path:
                Path to the drift profile to use for monitoring. If provided, and drift profile is not provided,
                the drift profile will be loaded from the path.

            profile_request:
                If both the drift_profile and drift_profile_path are not provided, the profile request object will be used
                to construct a query string that is used to call the scouter server and obtain the drift profile.

            scouter_server_config:
                An optional HTTPConfig object to pass if the necessary scouter_server environment variables are not set;
                for example, SCOUTER_SERVER_URL.
        """

    @property
    def id(self) -> str:
        """Return the id."""

    @property
    def drift_profile(self) -> Union[SpcDriftProfile, PsiDriftProfile]:
        """Return the drift profile."""

    @property
    def config(self) -> Union[KafkaConfig, HTTPConfig, RabbitMQConfig]:
        """Return the configuration object."""

class ScouterQueue:
    def __init__(
        self,
        transport_config: DriftTransportConfig,
    ) -> None:
        """Scouter monitoring queue.

        Args:
            transport_config:
                Configuration object containing profile and producer configuration.
        """

    def insert(self, entity: Features | Metrics) -> None:
        """Insert features into the queue.

        Args:
            entity:
                Features or metrics to insert into the monitoring queue.
        """

    def flush(self) -> None:
        """Flush the queue."""

class SpcServerRecord:
    def __init__(
        self,
        repository: str,
        name: str,
        version: str,
        feature: str,
        value: float,
    ):
        """Initialize spc drift server record

        Args:
            repository:
                Model repository
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
    def repository(self) -> str:
        """Return the repository."""

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
        repository: str,
        name: str,
        version: str,
        feature: str,
        bin_id: str,
        bin_count: int,
    ):
        """Initialize spc drift server record

        Args:
            repository:
                Model repository
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
    def repository(self) -> str:
        """Return the repository."""

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
    def bin_id(self) -> str:
        """Return the sample value."""

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
        repository: str,
        name: str,
        version: str,
        metric: str,
        value: int,
    ):
        """Initialize spc drift server record

        Args:
            repository:
                Model repository
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
    def repository(self) -> str:
        """Return the repository."""

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

class Features:
    def __init__(self, features: List[Feature]) -> None:
        """Initialize features

        Args:
            features:
                List of features
        """

    def __str__(self) -> str:
        """Return the string representation of the features"""

class Metric:
    def __init__(self, name: str, value: float) -> None:
        """Initialize metric

        Args:
            name:
                Name of the metric
            value:
                Value to assign to the metric
        """

    def __str__(self) -> str:
        """Return the string representation of the metric"""

class Metrics:
    def __init__(self, metrics: List[Metric]) -> None:
        """Initialize metrics

        Args:
            metrics:
                List of metrics
        """

    def __str__(self) -> str:
        """Return the string representation of the metrics"""

class PsiFeatureQueue:
    def __init__(self, drift_profile: PsiDriftProfile) -> None:
        """Initialize the feature queue

        Args:
            drift_profile:
                Drift profile to use for feature queue.
        """

    def insert(self, features: Features) -> None:
        """Insert data into the feature queue
        Args:
            features:
                List of features to insert into the monitoring queue.
        """

    def is_empty(self) -> bool:
        """check if queue is empty
        Returns:
            bool
        """

    def clear_queue(self) -> None:
        """Clears the feature queue"""

    def create_drift_records(self) -> ServerRecords:
        """Create drift server record from data


        Returns:
            `DriftServerRecord`
        """

class SpcFeatureQueue:
    def __init__(self, drift_profile: SpcDriftProfile) -> None:
        """Initialize the feature queue

        Args:
            drift_profile:
                Drift profile to use for feature queue.
        """

    def insert(self, features: Features) -> None:
        """Insert data into the feature queue

        Args:
            features:
                List of features to insert into the monitoring queue.
        """

    def create_drift_records(self) -> ServerRecords:
        """Create drift server record from data


        Returns:
            `DriftServerRecord`
        """

    def clear_queue(self) -> None:
        """Clears the feature queue"""

class CustomMetricFeatureQueue:
    def __init__(self, drift_profile: CustomDriftProfile) -> None:
        """Initialize the feature queue

        Args:
            drift_profile:
                Drift profile to use for feature queue.
        """

    def insert(self, metrics: Metrics) -> None:
        """Insert data into the feature queue

        Args:
            metrics:
                List of metrics to insert into the monitoring queue.
        """

    def create_drift_records(self) -> ServerRecords:
        """Create drift server record from data


        Returns:
            `DriftServerRecord`
        """

    def clear_queue(self) -> None:
        """Clears the feature queue"""

    def is_empty(self) -> bool:
        """check if queue is empty
        Returns:
            bool
        """
