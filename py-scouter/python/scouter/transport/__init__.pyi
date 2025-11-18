# pylint: disable=dangerous-default-value
from typing import Dict, Optional

from ..logging import LogLevel

class TransportType:
    Kafka = "TransportType"
    RabbitMQ = "TransportType"
    Redis = "TransportType"
    HTTP = "TransportType"

class HTTPConfig:
    server_uri: str
    username: str
    password: str
    auth_token: str

    def __init__(
        self,
        server_uri: Optional[str] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        auth_token: Optional[str] = None,
    ) -> None:
        """HTTP configuration to use with the HTTPProducer.

        Args:
            server_uri:
                URL of the HTTP server to publish messages to.
                If not provided, the value of the HTTP_server_uri environment variable is used.

            username:
                Username for basic authentication.

            password:
                Password for basic authentication.

            auth_token:
                Authorization token to use for authentication.

        """

    def __str__(self): ...

class KafkaConfig:
    brokers: str
    topic: str
    compression_type: str
    message_timeout_ms: int
    message_max_bytes: int
    log_level: LogLevel
    config: Dict[str, str]
    max_retries: int
    transport_type: TransportType

    def __init__(
        self,
        username: Optional[str] = None,
        password: Optional[str] = None,
        brokers: Optional[str] = None,
        topic: Optional[str] = None,
        compression_type: Optional[str] = None,
        message_timeout_ms: int = 600_000,
        message_max_bytes: int = 2097164,
        log_level: LogLevel = LogLevel.Info,
        config: Dict[str, str] = {},
        max_retries: int = 3,
    ) -> None:
        """Kafka configuration for connecting to and publishing messages to Kafka brokers.

        This configuration supports both authenticated (SASL) and unauthenticated connections.
        When credentials are provided, SASL authentication is automatically enabled with
        secure defaults.

        Authentication Priority (first match wins):
            1. Direct parameters (username/password)
            2. Environment variables (KAFKA_USERNAME/KAFKA_PASSWORD)
            3. Configuration dictionary (sasl.username/sasl.password)

        SASL Security Defaults:
            - security.protocol: "SASL_SSL" (override via KAFKA_SECURITY_PROTOCOL env var)
            - sasl.mechanism: "PLAIN" (override via KAFKA_SASL_MECHANISM env var)

        Args:
            username:
                SASL username for authentication.
                Fallback: KAFKA_USERNAME environment variable.
            password:
                SASL password for authentication.
                Fallback: KAFKA_PASSWORD environment variable.
            brokers:
                Comma-separated list of Kafka broker addresses (host:port).
                Fallback: KAFKA_BROKERS environment variable.
                Default: "localhost:9092"
            topic:
                Target Kafka topic for message publishing.
                Fallback: KAFKA_TOPIC environment variable.
                Default: "scouter_monitoring"
            compression_type:
                Message compression algorithm.
                Options: "none", "gzip", "snappy", "lz4", "zstd"
                Default: "gzip"
            message_timeout_ms:
                Maximum time to wait for message delivery (milliseconds).
                Default: 600000 (10 minutes)
            message_max_bytes:
                Maximum message size in bytes.
                Default: 2097164 (~2MB)
            log_level:
                Logging verbosity for the Kafka producer.
                Default: LogLevel.Info
            config:
                Additional Kafka producer configuration parameters.
                See: https://kafka.apache.org/documentation/#producerconfigs
                Note: Direct parameters take precedence over config dictionary values.
            max_retries:
                Maximum number of retry attempts for failed message deliveries.
                Default: 3

        Examples:
            Basic usage (unauthenticated):
            ```python
            config = KafkaConfig(
                brokers="kafka1:9092,kafka2:9092",
                topic="my_topic"
            )
            ```

            SASL authentication:
            ```python
            config = KafkaConfig(
                username="my_user",
                password="my_password",
                brokers="secure-kafka:9093",
                topic="secure_topic"
            )
            ```

            Advanced configuration:
            ```python
            config = KafkaConfig(
                brokers="kafka:9092",
                compression_type="lz4",
                config={
                    "acks": "all",
                    "batch.size": "32768",
                    "linger.ms": "10"
                }
            )
            ```
        """

    def __str__(self): ...

class RabbitMQConfig:
    address: str
    queue: str
    max_retries: int
    transport_type: TransportType

    def __init__(
        self,
        host: Optional[str] = None,
        port: Optional[int] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        queue: Optional[str] = None,
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

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.
        """

    def __str__(self): ...

class RedisConfig:
    address: str
    channel: str
    transport_type: TransportType

    def __init__(
        self,
        address: Optional[str] = None,
        chanel: Optional[str] = None,
    ) -> None:
        """Redis configuration to use with a Redis producer

        Args:
            address (str):
                Redis address.
                If not provided, the value of the REDIS_ADDR environment variable is used and defaults to
                "redis://localhost:6379".

            channel (str):
                Redis channel to publish messages to.

                If not provided, the value of the REDIS_CHANNEL environment variable is used and defaults to "scouter_monitoring".
        """

    def __str__(self): ...
