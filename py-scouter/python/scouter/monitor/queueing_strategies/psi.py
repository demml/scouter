import threading
import time
from typing import Any, Dict, Optional, Union

from scouter import (
    HTTPConfig,
    KafkaConfig,
    PsiDriftProfile,
    PsiFeatureQueue,
    RabbitMQConfig,
    ServerRecords,
)
from scouter.monitor.queueing_strategies.base import BaseQueueingStrategy
from scouter.utils.logger import ScouterLogger

logger = ScouterLogger.get_logger()

PSI_MAX_QUEUE_SIZE = 2


class PsiQueueingStrategy(BaseQueueingStrategy):
    def __init__(
        self,
        drift_profile: PsiDriftProfile,
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
        super().__init__(PsiFeatureQueue(drift_profile=drift_profile), config)
        self._drift_profile = drift_profile
        self._count = 0

    # def _run_queue_checker(self):
    #     thread = threading.Thread(target=self._check_queue)
    #     thread.daemon = True  # Ensure the thread exits when the main program does
    #     thread.start()
    #
    # def _check_queue(self):
    #     last_metrics_time = time.time()
    #     while True:
    #         try:
    #             # Check if 30 seconds have passed
    #             current_time = time.time()
    #             if current_time - last_metrics_time >= 30 and not self._feature_queue.is_empty():
    #
    #                 last_metrics_time = current_time
    #         except Exception as e:  # pylint: disable=broad-except
    #             logger.error("Error collecting metrics: {}", e)

    # def _clear_queue(self) -> None:
    #         """Clear the monitoring queue."""
    #         self._feature_queue.clear_queue()
    #         self._count = 0

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
            if self._count >= PSI_MAX_QUEUE_SIZE:
                return self._publish()
            return None

        except KeyError as exc:
            logger.error("Key error: {}", exc)
            return None

        except Exception as exc:
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
            return None

    # def _publish(self) -> None:
    #     """Publish drift records to the monitoring server."""
    #     pass
