from typing import Dict, Optional

from scouter import LogLevel

class KafkaConfig:
    brokers: str
    topic: str
    compression_type: str
    message_timeout_ms: int
    message_max_bytes: int
    log_level: LogLevel
    config: Dict[str, str]

    def __init__(
        self,
        brokers: Optional[str] = None,
        topic: Optional[str] = None,
        compression_type: Optional[str] = None,
        raise_one_error: bool = False,
        message_timeout_ms: int = 600_000,
        message_max_bytes: int = 2097164,
        log_level: LogLevel = LogLevel.Info,
        config: Dict[str, str] = {},
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
                See https://kafka.apache.org/documentation/#configuration

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

    def __init__(
        self,
        address: Optional[str] = None,
        queue: Optional[str] = None,
        raise_on_error: bool = False,
    ) -> None:
        """RabbitMQ configuration to use with the RabbitMQProducer.

        Args:
            address:
                RabbitMQ address.
                If not provided, the value of the RABBITMQ_ADDRESS environment variable is used.

            queue:
                RabbitMQ queue to publish messages to.
                If not provided, the value of the RABBITMQ_QUEUE environment variable is used.

            raise_on_error:
                Whether to raise an error if message delivery fails.
                Default is False.
        """
