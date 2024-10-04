import threading
import time
from queue import Empty, Queue
from typing import Optional, Tuple, Union
from uuid import uuid4

from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig
from scouter.integrations.kafka import KafkaConfig
from scouter.integrations.producer import DriftRecordProducer
from scouter.integrations.rabbitmq import RabbitMQConfig
from scouter.utils.logger import ScouterLogger

from .._scouter import Observer, ServerRecords

logger = ScouterLogger.get_logger()


def generate_queue_id() -> str:
    return uuid4().hex


_queue_id = generate_queue_id()


class ScouterObserver:
    """Instantiates an api observer to collect metrics and publish them to the monitoring server."""

    def __init__(
        self,
        repository: str,
        name: str,
        version: str,
        config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig],
    ):
        """Initializes an api metric observer. Upon instantiation, a queue and producer will be initialized.
        Un addition, the requests send to the queue will be processed and published via a separate thread.

        Args:
            repository:
                Model repository
            name:
                Model name
            version:
                Model version
            config:
                Configuration for the monitoring producer. The configured producer
                will be used to publish drift records to the monitoring server.
        """
        self._queue: Queue[Tuple[str, float, str, int]] = Queue()
        self._observer = Observer(repository, name, version)
        self._running = True
        self._thread = threading.Thread(target=self._process_queue)
        self._thread.daemon = True  # Ensure the thread exits when the main program does
        self._thread.start()

        self._producer = self._get_producer(config)
        logger.info("Queue and producer initialized")

    def _get_producer(self, config: Union[KafkaConfig, HTTPConfig, RabbitMQConfig]) -> BaseProducer:
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

            except Exception as e:
                logger.error("Error processing queue: {}", e)
                pass

            try:
                # Check if 30 seconds have passed
                current_time = time.time()
                if current_time - last_metrics_time >= 30:
                    metrics: Optional[ServerRecords] = self._observer.collect_metrics()

                    if metrics:
                        self._producer.publish(metrics)

                    self._observer.reset_metrics()
                    last_metrics_time = current_time
            except Exception as e:
                logger.error("Error collecting metrics: {}", e)

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
        self._producer.flush()
        self._queue.put((_queue_id, 0, "", 0))
        self._thread.join()
