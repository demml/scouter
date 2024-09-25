# pylint: disable=pointless-statement,broad-exception-caught


from typing import Any, Dict, Optional, Union

from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.utils.logger import ScouterLogger

from ._scouter import (  # pylint: disable=no-name-in-module
    CommonCron,
    DriftProfile,
    DriftServerRecords,
    FeatureQueue,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


class MonitorQueue:
    def __init__(
        self,
        drift_profile: DriftProfile,
        config: Union[KafkaConfig, HTTPConfig],
    ) -> None:
        """Instantiate a monitoring queue to monitor data drift.

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        self._drift_profile = drift_profile

        logger.info("Initializing queue and producer")
        self.feature_queue = FeatureQueue(drift_profile=drift_profile)
        self._count = 0

        self._producer = self._get_producer(config)
        logger.info("Queue and producer initialized")

    def _get_producer(self, config: Union[KafkaConfig, HTTPConfig]) -> BaseProducer:
        """Get the producer based on the configuration."""
        return DriftRecordProducer.get_producer(config)

    def insert(self, data: Dict[Any, Any]) -> Optional[DriftServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
        try:
            self.feature_queue.insert(data)
            self._count += 1

            if self._count >= self._drift_profile.config.sample_size:
                return self.publish()

            return None

        except KeyError as exc:
            logger.error("Key error: {}", exc)
            return None

        except Exception as exc:
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
            return None

    def _clear_queue(self) -> None:
        """Clear the monitoring queue."""
        self.feature_queue.clear_queue()
        self._count = 0

    def publish(self) -> DriftServerRecords:
        """Publish drift records to the monitoring server."""
        try:
            drift_records = self.feature_queue.create_drift_records()

            self._producer.publish(drift_records)

            # clear items
            self._clear_queue()

            return drift_records

        except Exception as exc:
            logger.error("Failed to compute drift: {}", exc)
            raise ValueError(f"Failed to compute drift: {exc}") from exc

    def flush(self) -> None:
        """Flush the producer."""
        self._producer.flush()
