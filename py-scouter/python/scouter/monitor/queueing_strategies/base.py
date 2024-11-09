from typing import Any, Dict, Union

from scouter import (
    DriftRecordProducer,
    HTTPConfig,
    KafkaConfig,
    PsiFeatureQueue,
    RabbitMQConfig,
    SpcFeatureQueue,
)
from scouter.utils.logger import ScouterLogger

logger = ScouterLogger.get_logger()


class BaseQueueingStrategy:
    def __init__(
        self,
        feature_queue: Union[SpcFeatureQueue, PsiFeatureQueue],
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ) -> None:
        """Abstract base class that defines the core structure and shared functionality
        for queueing strategies

        This class provides foundational logic for initializing a producer and enforces
        the implementation of essential methods like `publish` and `insert`. It is intended
        to be extended by specific queueing strategies, such as those for SPC and PSI, which
        can then be managed by the MonitorQueue context manager.

        Args:
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        self._producer = DriftRecordProducer.get_producer(config)
        self._feature_queue = feature_queue

    def _clear_queue(self) -> None:
        """Clear the monitoring queue."""
        self._feature_queue.clear_queue()
        self._count = 0

    def _publish(self) -> None:
        """Publish drift records to the monitoring server."""
        try:
            drift_records = self._feature_queue.create_drift_records()

            self._producer.publish(drift_records)
            # clear items
            self._clear_queue()

        except Exception as exc:
            logger.error("Failed to compute drift: {}", exc)
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def flush(self) -> None:
        """Flush the producer."""
        self._producer.flush()

    def insert(self, data: Dict[Any, Any]) -> None:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
        raise NotImplementedError
