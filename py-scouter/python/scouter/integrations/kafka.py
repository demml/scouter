from typing import Optional, Literal, Dict, Any, List
from pydantic import BaseModel, field_validator
import os
from confluent_kafka import Message, Producer, error
from scouter import DriftServerRecord
import tenacity
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes
from scouter.integrations.base import BaseProducer

logger = ScouterLogger.get_logger()
MESSAGE_MAX_BYTES_DEFAULT = 2097164


class KafkaConfig:
    brokers: str
    topic: str
    compression_type: Optional[
        Literal[None, "gzip", "snappy", "lz4", "zstd", "inherit"]
    ] = "gzip"
    username: Optional[str] = None
    password: Optional[str] = None
    raise_on_err: bool = True
    message_timeout_ms: int = 600_000
    message_max_bytes: int = MESSAGE_MAX_BYTES_DEFAULT

    @field_validator("brokers", mode="before")
    def check_brokers(cls, v, values) -> str:
        if v is None:
            v = os.getenv("KAFKA_BROKERS", "localhost:9092")
        return v

    @field_validator("topic", mode="before")
    def check_topic(cls, v, values) -> str:
        if v is None:
            v = os.getenv("KAFKA_TOPIC", "scouter_monitoring")

        return v

    @field_validator("username", mode="before")
    def check_username(cls, v, values) -> Optional[str]:
        if v is None:
            v = os.getenv("KAFKA_USERNAME", None)
        return v

    @field_validator("password", mode="before")
    def check_password(cls, v, values) -> Optional[str]:
        if v is None:
            v = os.getenv("KAFKA_PASSWORD", None)
        return v

    @property
    def has_credentials(self) -> bool:
        return bool(self.username) and bool(self.password)


class KafkaProducer(BaseProducer):
    def __init__(self, config: KafkaConfig, max_retries: int = 3):
        self.config = config

        self.max_retries = max_retries
        self._kafka_config = {
            "bootstrap.servers": self.config.brokers,
            "compression.type": self.config.compression_type,
            "message.timeout.ms": self.config.message_timeout_ms,
            "message.max.bytes": self.config.message_max_bytes,
        }

        if self.config.has_credentials:
            self._kafka_config["sasl.username"] = self.config.username
            self._kafka_config["sasl.password"] = self.config.password
            self._kafka_config["security.protocol"] = "SASL_SSL"
            self._kafka_config["sasl.mechanisms"] = "PLAIN"

        self._producer = Producer(self._kafka_config)

    def _delivery_callback(err):
        """Called once for each message produced to indicate delivery result.
        Triggered by poll() or flush()."""

        if err is not None:
            logger.error("Message delivery failed: {}", err)
        else:
            pass

    def _publish(self, records: List[DriftServerRecord]) -> None:
        for record in records:
            try:
                self._producer.produce(
                    self.config.topic,
                    record.model_dump_json(),
                    callback=self._delivery_callback,
                )
                logger.debug(f"Sent to topic: {self.config.topic}")
                self._producer.poll(0)

            except Exception as e:
                logger.error(f"Could not send message to Kafka due to: {e}")
                if self.config.raise_on_err:
                    raise error.ProduceError(
                        f"Could not send message to Kafka due to: {e}"
                    )

    def publish(self, records: List[DriftServerRecord]) -> None:
        """Publishes drift records to a kafka topic with retries.

        If the message delivery fails, the message is retried up to `max_retries` times before raising an error.

        Args:
            records:
                List of drift records to publish.

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

        retrier(records)

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
