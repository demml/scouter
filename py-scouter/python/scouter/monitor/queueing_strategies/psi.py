import threading
import time
from typing import Union

from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.monitor.queueing_strategies.base import BaseQueueingStrategy
from scouter.utils.logger import ScouterLogger
from typing_extensions import Optional

from ..._scouter import (  # pylint: disable=no-name-in-module
    Features,
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
        self._stop_event = threading.Event()
        self._activate_queue_observer()

    def _activate_queue_observer(self) -> None:
        self._stop_event.clear()
        thread = threading.Thread(target=self._queue_observer)
        thread.daemon = True  # Ensure the thread exits when the main program does
        thread.start()

    def stop_queue_observer(self) -> None:
        """Stop the queue observer thread."""
        self._stop_event.set()

    def _queue_observer(self) -> None:
        last_metrics_time = time.time()
        while not self._stop_event.is_set():
            try:
                current_time = time.time()
                if current_time - last_metrics_time >= 30.0 and not self._feature_queue.is_empty():
                    _ = self._publish(self._feature_queue)
                    last_metrics_time = current_time
            except Exception as e:  # pylint: disable=broad-except
                logger.error("Error collecting metrics: {}", e)

    def insert(self, features: Features) -> Optional[ServerRecords]:
        """Insert data into the monitoring queue.

        Args:
            features:
                List of features to insert into the monitoring queue.

        Returns:
            ServerRecords: The server records if the queue is full and the data is published.
        """
        try:
            self._feature_queue.insert(features)
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
