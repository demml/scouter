from typing import Optional, Union

from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.monitor.queueing_strategies.base import BaseQueueingStrategy
from scouter.utils.logger import ScouterLogger

from ..._scouter import (  # pylint: disable=no-name-in-module
    Features,
    ServerRecords,
    SpcDriftProfile,
    SpcFeatureQueue,
)

logger = ScouterLogger.get_logger()


class SpcQueueingStrategy(BaseQueueingStrategy):
    def __init__(
        self,
        drift_profile: SpcDriftProfile,
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ) -> None:
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
        self._feature_queue = SpcFeatureQueue(drift_profile=drift_profile)
        self._drift_profile = drift_profile

    def insert(self, features: Features) -> Optional[ServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            features:
                List of features to insert into the monitoring queue.

        Returns:
            ServerRecords: The drift records published to the monitoring server.
        """
        try:
            self._feature_queue.insert(features)
            self._count += 1
            if self._count >= self._drift_profile.config.sample_size:
                return self._publish(self._feature_queue)
            return None
        except KeyError as exc:
            logger.error("Key error: {}", exc)
            return None

        except Exception as exc:  # pylint: disable=W0718
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
            return None
