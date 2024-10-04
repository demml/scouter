import threading
import time
from queue import Empty, Queue
from typing import Optional, Tuple

from .._scouter import ObservabilityMetrics, Observer


class ScouterObserver:
    def __init__(self, repository: str, name: str, version: str):
        self._queue: Queue[Tuple[str, float, str, int]] = Queue()
        self._observer = Observer(repository, name, version)
        self._running = True
        self._thread = threading.Thread(target=self._process_queue)
        self._thread.daemon = True  # Ensure the thread exits when the main program does
        self._thread.start()

    def _process_queue(self) -> None:
        last_metrics_time = time.time()
        while self._running:
            try:
                request: Tuple[str, float, str, int] = self._queue.get(timeout=1)
                self._observer.increment(request[0], request[1], request[2], request[3])
                self._queue.task_done()
            except Empty:
                pass

            # Check if 30 seconds have passed
            current_time = time.time()
            if current_time - last_metrics_time >= 30:
                metrics: Optional[ObservabilityMetrics] = self._observer.collect_metrics()

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
        self._queue.put(None)
        self._thread.join()
