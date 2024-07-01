from typing import Optional, Literal, Dict, Any
from pydantic import field_validator, model_validator, BaseModel
import os
from scouter import DriftServerRecord
import tenacity
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes
from scouter.integrations.base import BaseProducer
from typing_extensions import Self

logger = ScouterLogger.get_logger()
MESSAGE_MAX_BYTES_DEFAULT = 2097164


class KafkaConfig(BaseModel):
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

        raise_on_err:
            Whether to raise an error if message delivery fails.
            Default is True.

        message_timeout_ms:
            Message timeout in milliseconds.
            Default is 600_000.

        message_max_bytes:
            Maximum message size in bytes.
            Default is 2097164.

        config:
            Additional Kafka configuration options. These will be passed to the Kafka producer.
            See https://kafka.apache.org/documentation/#configuration

    """

    brokers: str
    topic: str
    compression_type: Optional[
        Literal[None, "gzip", "snappy", "lz4", "zstd", "inherit"]
    ] = "gzip"
    raise_on_err: bool = True
    message_timeout_ms: int = 600_000
    message_max_bytes: int = MESSAGE_MAX_BYTES_DEFAULT
    config: Dict[str, Any] = {}

    @field_validator("brokers", mode="before")
    @classmethod
    def check_brokers(cls, v, values) -> str:
        if v is None:
            v = os.getenv("KAFKA_BROKERS", "localhost:9092")
        return v

    @field_validator("topic", mode="before")
    @classmethod
    def check_topic(cls, v, values) -> str:
        if v is None:
            v = os.getenv("KAFKA_TOPIC", "scouter_monitoring")

        return v

    @model_validator(mode="after")
    def finalize_config(self) -> Self:
        """Finalizes the kafka configuration by checking and setting credentials if provided."""

        if not all(key in self.config for key in ["sasl.username", "sasl.password"]):
            sasl_username = os.getenv("KAFKA_SASL_USERNAME")
            sasl_password = os.getenv("KAFKA_SASL_PASSWORD")
            if (sasl_username is not None) and (sasl_password is not None):
                logger.info(
                    """KAFKA_SASL_USERNAME and KAFKA_SASL_PASSWORD found in environment. 
                    Assigning security.protocol and sasl.mechanism"""
                )
                self.config["sasl.username"] = sasl_username
                self.config["sasl.password"] = sasl_password
                self.config["security.protocol"] = "SASL_SSL"
                self.config["sasl.mechanisms"] = "PLAIN"

        # set default values
        self.config["bootstrap.servers"] = self.brokers
        self.config["compression.type"] = self.compression_type
        self.config["message.timeout.ms"] = self.message_timeout_ms
        self.config["message.max.bytes"] = self.message_max_bytes

        return self


class KafkaProducer(BaseProducer):
    def __init__(
        self,
        config: KafkaConfig,
        max_retries: int = 3,
    ):
        """Kafka producer to publish drift records to a Kafka topic.

        Args:
            config:
                Kafka configuration to use.
            max_retries:
                Maximum number of retries to attempt if message delivery fails.
        """
        self._kafka_config = config
        self.max_retries = max_retries

        # Should fail on instantiation if the kafka library is not installed
        try:
            from confluent_kafka import Producer

            self._producer = Producer(self._kafka_config.config)

        except ModuleNotFoundError as e:
            logger.error(
                "Could not import confluent_kafka. Please install it using: pip install 'scouter[kafka]'"
            )
            raise e

    def _delivery_callback(err) -> None:
        """Called once for each message produced to indicate delivery result.
        Triggered by poll() or flush()."""

        if err is not None:
            logger.error("Message delivery failed: {}", err)
        else:
            pass

    def _publish(self, record: DriftServerRecord) -> None:
        try:
            self._producer.produce(
                topic=self._kafka_config.topic,
                value=record.model_dump_json(),
                on_delivery=self._delivery_callback,  # type: ignore
            )
            logger.debug(f"Sent to topic: {self._kafka_config.topic}")
            self._producer.poll(0)

        except Exception as e:
            logger.error(f"Could not send message to Kafka due to: {e}")
            if self._kafka_config.raise_on_err:
                raise e

    def publish(self, record: DriftServerRecord) -> None:
        """Publishes drift record to a kafka topic with retries.

        If the message delivery fails, the message is retried up to `max_retries` times before raising an error.

        Args:
            record:


        Raises:
            ValueError: When max_retries is invalid.
        """
        if self.max_retries < 1:
            raise ValueError("max_retries must be 1 or greater")

        retrier = tenacity.retry(
            wait=tenacity.wait_exponential(min=1, max=16),
            stop=tenacity.stop_after_attempt(self.max_retries),
            reraise=True,
        )(self._publish)

        retrier(record)

    def flush(self, timeout: Optional[float] = None) -> None:
        if timeout is None:
            self._producer.flush()
            return

        num_remaining = self._producer.flush(timeout=timeout)
        if num_remaining > 0:
            logger.warning(
                "flush timed out with %s messages remaining. Undelivered messages will be discarded.",
                num_remaining,
            )

    @staticmethod
    def type() -> str:
        return ProducerTypes.Kafka.value