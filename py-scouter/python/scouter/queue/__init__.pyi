from typing import Dict, Optional, Union

from scouter import Features, LogLevel, PsiDriftProfile, ServerRecords, SpcDriftProfile

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
        max_retries: int = 3
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

        ...

class HTTPConfig:
    server_url: str
    use_auth: bool
    username: str
    password: str
    auth_token: str

    def __init__(
        self,
        server_url: Optional[str] = None,
        use_auth: bool = False,
        username: Optional[str] = None,
        password: Optional[str] = None,
        auth_token: Optional[str] = None,
    ) -> None:
        """HTTP configuration to use with the HTTPProducer.

        Args:
            server_url:
                URL of the HTTP server to publish messages to.
                If not provided, the value of the HTTP_SERVER_URL environment variable is used.

            use_auth:
                Whether to use basic authentication.
                Default is False.

            username:
                Username for basic authentication.

            password:
                Password for basic authentication.

            auth_token:
                Authorization token to use for authentication.

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
        max_retries: int = 3
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

        ...

    def publish(self, message: ServerRecords) -> None:
        """Publish a message to the queue.

        Args:
            message:
                Message to publish.
        """

        ...

    def flush(self) -> None:
        """Flush the producer queue."""

        ...

class ScouterQueue:
    def __init__(
        self,
        drift_profile: Union[SpcDriftProfile, PsiDriftProfile],
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ) -> None:
        """Scouter monitoring queue.

        Args:
            drift_profile:
                Drift profile to use for monitoring.

            config:
                Configuration object for the queue that specifies the type of queue to use.

            max_retries:
                Maximum number of retries to attempt when publishing via the producer.
                Default is 3.
        """

        ...

    def insert(self, features: Features) -> None:
        """Insert features into the queue.

        Args:
            features:
                Features to insert.
        """

        ...

    def flush(self) -> None:
        """Flush the queue."""

        ...
