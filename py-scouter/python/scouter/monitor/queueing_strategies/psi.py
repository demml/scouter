import threading
import time
from typing import Any, Dict, Union

from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.monitor.queueing_strategies.base import BaseQueueingStrategy
from scouter.utils.logger import ScouterLogger
from typing_extensions import Optional

from ..._scouter import (  # pylint: disable=no-name-in-module
    PsiDriftProfile,
    PsiFeatureQueue,
    ServerRecords,
)

logger = ScouterLogger.get_logger()

PSI_MAX_QUEUE_SIZE = 1000


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
        super().__init__(config)
        self._feature_queue = PsiFeatureQueue(drift_profile=drift_profile)
        self._activate_queue_observer()

    def _activate_queue_observer(self) -> None:
        thread = threading.Thread(target=self._queue_observer)
        thread.daemon = True  # Ensure the thread exits when the main program does
        thread.start()

    def _queue_observer(self) -> None:
        last_metrics_time = time.time()
        while True:
            try:
                current_time = time.time()
                if current_time - last_metrics_time >= 30.0 and not self._feature_queue.is_empty():
                    _ = self._publish(self._feature_queue)
                    last_metrics_time = current_time
            except Exception as e:  # pylint: disable=broad-except
                logger.error("Error collecting metrics: {}", e)

    def insert(self, data: Dict[Any, Any]) -> Optional[ServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            data:
                Dictionary of feature values to insert into the monitoring queue.
        """
        try:
            self._feature_queue.insert(data)
            self._count += 1
            if self._count >= PSI_MAX_QUEUE_SIZE:
                return self._publish(self._feature_queue)
            return None
        except KeyError as exc:
            logger.error("Key error: {}", exc)
            return None

        except Exception as exc:  # pylint: disable=W0718
            logger.error("Failed to insert data into monitoring queue: {}. Passing", exc)
            return None
