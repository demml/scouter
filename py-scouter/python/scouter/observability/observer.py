import threading
import time
from queue import Empty, Queue
from typing import Optional, Tuple, Union
from uuid import uuid4
from .._scouter import ObservabilityMetrics, Observer

from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.utils.logger import ScouterLogger
from scouter.integrations.producer import DriftRecordProducer

logger = ScouterLogger.get_logger()


def generate_queue_id() -> str:
    return uuid4().hex


_queue_id = generate_queue_id()


class ScouterObserver:
    def __init__(
        self,
        repository: str,
        name: str,
        version: str,
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ):
        self._queue: Queue[Tuple[str, float, str, int]] = Queue()
        self._observer = Observer(repository, name, version)
        self._running = True
        self._thread = threading.Thread(target=self._process_queue)
        self._thread.daemon = True  # Ensure the thread exits when the main program does
        self._thread.start()

        self._producer = self._get_producer(config)
        logger.info("Queue and producer initialized")

    def _get_producer(
        self, config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig]
    ) -> BaseProducer:
        """Get the producer based on the configuration."""
        return DriftRecordProducer.get_producer(config)

    def _process_queue(self) -> None:
        last_metrics_time = time.time()
        while self._running:
            try:
                request: Tuple[str, float, str, int] = self._queue.get(timeout=1)

                if request[0] == _queue_id:
                    self._running = False
                    break

                self._observer.increment(request[0], request[1], request[2], request[3])
                self._queue.task_done()
            except Empty:
                pass

            # Check if 30 seconds have passed
            current_time = time.time()
            if current_time - last_metrics_time >= 30:
                metrics: Optional[
                    ObservabilityMetrics
                ] = self._observer.collect_metrics()

                if metrics:
                    print(f"Metrics: {metrics.request_count}")
                    print("push to monitoring server")

                self._observer.reset_metrics()
                last_metrics_time = current_time

    def add_request_metrics(
        self,
        route: str,
        latency: float,
        status: str,
        status_code: int,
    ):
        """Add request metrics to the observer

        Args:
            route:
                Route
            latency:
                Latency
            status:
                Status
            status_code:
                Status code
        """

        request = (route, latency, status, status_code)
        self._queue.put(request)

    def stop(self):
        self._queue.put((_queue_id, 0, "", 0))
        self._thread.join()
