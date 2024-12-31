# pylint: disable=pointless-statement,broad-exception-caught
from typing import Optional, Union

from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.monitor.queueing_strategies.psi import PsiQueueingStrategy
from scouter.monitor.queueing_strategies.spc import SpcQueueingStrategy
from scouter.utils.logger import ScouterLogger

from .._scouter import (  # pylint: disable=no-name-in-module
    CommonCron,
    Features,
    PsiDriftProfile,
    ServerRecords,
    SpcDriftProfile,
)

logger = ScouterLogger.get_logger()

CommonCrons = CommonCron()  # type: ignore


def _get_queueing_strategy(
    drift_profile: Union[SpcDriftProfile, PsiDriftProfile],
    config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
) -> Union[SpcQueueingStrategy, PsiQueueingStrategy]:
    """Get the feature queue based on the drift profile.

    Args:
        drift_profile:
            Monitoring profile containing feature drift profiles.
    """
    if isinstance(drift_profile, SpcDriftProfile):
        return SpcQueueingStrategy(drift_profile=drift_profile, config=config)
    if isinstance(drift_profile, PsiDriftProfile):
        return PsiQueueingStrategy(drift_profile=drift_profile, config=config)
    raise ValueError(f"Drift type {drift_profile.config.drift_type} not supported")


class MonitorQueue:
    def __init__(
        self,
        drift_profile: Union[SpcDriftProfile, PsiDriftProfile],
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ) -> None:
        """Instantiate a monitoring queue to monitor data drift.

        Args:
            drift_profile:
                Monitoring profile containing feature drift profiles.
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        logger.info("Initializing queue and producer")
        self._queueing_strategy = _get_queueing_strategy(drift_profile, config)
        logger.info("Queue and producer initialized")

    def insert(self, features: Features) -> Optional[ServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            features:
                List of features to insert into the monitoring queue.

        Returns:
            List of drift records if queueing strategy conditions are met
        """
        return self._queueing_strategy.insert(features)

    def flush(self) -> None:
        """Flush the producer."""
        return self._queueing_strategy.flush()
