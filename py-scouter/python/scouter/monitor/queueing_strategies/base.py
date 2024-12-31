from abc import ABC, abstractmethod
from typing import Optional, Union

from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.utils.logger import ScouterLogger

from ..._scouter import (  # pylint: disable=no-name-in-module
    Features,
    PsiFeatureQueue,
    ServerRecords,
    SpcFeatureQueue,
)

logger = ScouterLogger.get_logger()


class BaseQueueingStrategy(ABC):
    def __init__(self, config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig]) -> None:
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
        self._count = 0

    def _clear_queue(self, feature_queue: Union[PsiFeatureQueue, SpcFeatureQueue]) -> None:
        """Clear the monitoring queue."""
        feature_queue.clear_queue()
        self._count = 0

    def _publish(self, feature_queue: Union[PsiFeatureQueue, SpcFeatureQueue]) -> ServerRecords:
        """Publish drift records to the monitoring server."""
        try:
            drift_records = feature_queue.create_drift_records()

            self._producer.publish(drift_records)

            self._clear_queue(feature_queue)

            return drift_records
        except Exception as exc:
            logger.error("Failed to compute drift: {}", exc)
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def flush(self) -> None:
        """Flush the producer."""
        self._producer.flush()

    @abstractmethod
    def insert(self, features: Features) -> Optional[ServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            features:
                List of features to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
