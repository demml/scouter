from typing import Any, Dict, Optional, Union

from scouter import (
    HTTPConfig,
    KafkaConfig,
    RabbitMQConfig,
    ServerRecords,
    SpcDriftProfile,
    SpcFeatureQueue,
)
from scouter.monitor.queueing_strategies.base import BaseQueueingStrategy
from scouter.utils.logger import ScouterLogger

logger = ScouterLogger.get_logger()


class SpcQueueingStrategy(BaseQueueingStrategy):
    def __init__(self, drift_profile: SpcDriftProfile, config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig]) -> None:
        """Initializes an SPC-specific queueing strategy with customized logic for inserting and
        publishing data related to statistical process control (SPC).

        This strategy manages the SPC drift profile and handles the queueing and publication
        of records for monitoring purposes.

        Args:
            drift_profile:
                Monitoring profile containing SPC feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        super().__init__(config)
        self._drift_profile = drift_profile
        self._feature_queue = SpcFeatureQueue(drift_profile=drift_profile)

    def insert(self, data: Dict[Any, Any]) -> None:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.

        Returns:
            List of drift records if the monitoring queue has enough data to compute
        """
        try:
            self._feature_queue.insert(data)
            self._count += 1
            if self._count >= self._drift_profile.config.sample_size:
                self._publish(self._feature_queue)

        except KeyError as exc:
            logger.error("Key error: {}", exc)

        except Exception as exc:
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
