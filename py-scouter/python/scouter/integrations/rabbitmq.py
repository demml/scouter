from typing import Any, Optional

import tenacity
from pydantic import BaseModel, field_validator
from scouter.integrations.base import BaseProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes

from .._scouter import DriftServerRecords

logger = ScouterLogger.get_logger()

ConnectionParameters = Any


class RabbitMQConfig(BaseModel):
    connection_params: ConnectionParameters
    queue: str = "scouter_monitoring"
    raise_on_err: bool = True

    @field_validator("connection_params", mode="before")
    @classmethod
    def _validate_connection_params(cls, v, values) -> ConnectionParameters:
        return 10
        from pika import ConnectionParameters  # type: ignore

        if v is None:
            return ConnectionParameters(host="localhost")

        assert isinstance(v, ConnectionParameters)  # type: ignore
        return v

    @property
    def type(self) -> str:
        return ProducerTypes.RabbitMQ.value


class RabbitMQProducer(BaseProducer):
    def __init__(
        self,
        config: RabbitMQConfig,
        max_retries: int = 3,
    ):
        """Kafka producer to publish drift records to a Kafka topic.

        Args:
            config:
                Kafka configuration to use.
            max_retries:
                Maximum number of retries to attempt if message delivery fails.
        """
        self._rabbit_config = config
        self.max_retries = max_retries

        try:
            from pika import BlockingConnection  # type: ignore
            from pika.adapters.blocking_connection import (  # type: ignore
                BlockingChannel,
            )

            connection = BlockingConnection(self._rabbit_config.connection_params)
            self._channel: BlockingChannel = connection.channel()
            self._channel.queue_declare(queue=self._rabbit_config.queue)

        except ModuleNotFoundError as e:
            logger.error(
                "Could not import confluent_kafka. Please install it using: pip install 'scouter[kafka]'"
            )
            raise e

    def _publish(self, records: DriftServerRecords) -> None:
        """Attempt to publish a message to the kafka broker.

        Args:
            records:
                Drift records to publish to the kafka broker.
        """
        try:
            self._channel.basic_publish(
                exchange="",
                routing_key=self._rabbit_config.queue,
                body=records.model_dump_json(),
            )
        except Exception as e:
            logger.error(f"Failed to publish message: {e}")
            if self._rabbit_config.raise_on_err:
                raise e

    def publish(self, records: DriftServerRecords) -> None:
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

        retrier(records)

    def flush(self, timeout: Optional[float] = None) -> None:
        """Flushes the producer to ensure all messages are sent."""

        if timeout is None:
            self._channel.close()
            return

        self._channel.close()
