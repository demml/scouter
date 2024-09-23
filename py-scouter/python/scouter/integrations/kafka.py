from functools import partial
from typing import Any, Optional

import tenacity
from scouter.integrations.base import BaseProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes

from .._scouter import DriftServerRecords, KafkaConfig

logger = ScouterLogger.get_logger()
MESSAGE_MAX_BYTES_DEFAULT = 2097164


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
            logger.error("Could not import confluent_kafka. Please install it using: pip install 'scouter[kafka]'")
            raise e

    def _delivery_report(self, err: Optional[str], msg: Any, raise_on_err: bool = True) -> None:
        """Callback acknowledging receipt of message from producer

        Args:
            err: error message
            msg: kafka message
            raise_on_err: whether to raise an error on failed message delivery. Default True.

        Raises:
            ProduceError: When message delivery to the kafka broker fails and raise_on_err is True.
        """
        if err is not None:
            err_data = {
                "kafka_message": msg.value(),
                "kafka_error": err,
            }
            err_msg = f"Failed delivery to topic: {msg.topic()}"
            logger.error("Failed delivery to topic: {} error_data: {}", msg.topic(), err_data)
            if raise_on_err:
                raise ValueError(err_msg)
        else:
            logger.debug(
                "Successful delivery to topic: %s, partition: %d, offset: %d",
                msg.topic(),
                msg.partition(),
                msg.offset(),
            )

    def _publish(self, records: DriftServerRecords) -> None:
        try:
            self._producer.produce(
                topic=self._kafka_config.topic,
                value=records.model_dump_json(),
                on_delivery=partial(
                    self._delivery_report, raise_on_err=self._kafka_config.raise_on_err
                ),  # type: ignore
            )
            logger.debug(f"Sent to topic: {self._kafka_config.topic}")
            self._producer.poll(0)

        except Exception as e:  # pylint: disable=broad-except
            logger.error(f"Could not send message to Kafka due to: {e}")
            if self._kafka_config.raise_on_err:
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
