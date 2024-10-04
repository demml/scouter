from unittest.mock import patch
from scouter import ObservabilityMetrics, ServerRecords
import time


def test_add_request(scouter_observer, mock_kafka_producer):
    scouter, mock_observer = scouter_observer
    scouter.add_request_metrics("route", 0.1, "OK", 200)
    assert not scouter._queue.empty()
    request = scouter._queue.get()
    assert request == ("route", 0.1, "OK", 200)


def test_process_queue(scouter_observer, mock_kafka_producer) -> None:
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.add_request_metrics("route", 0.1, "OK", 200)
    time.sleep(0.1)
    metrics: ServerRecords = scouter_observer._observer.collect_metrics()
    record = metrics.records[0].record

    assert isinstance(record, ObservabilityMetrics)
    assert record.request_count == 1
    scouter_observer.stop()


@patch("time.time", side_effect=[time.time() + 40])
def test_collect_and_reset_metrics(
    mock_time, scouter_observer, mock_kafka_producer
) -> None:
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.add_request_metrics("route", 0.1, "OK", 200)
    time.sleep(0.1)  # Give some time for the background thread to process the queue
    metrics = scouter_observer._observer.collect_metrics()

    # should be reset
    assert metrics is None


def test_stop(scouter_observer, mock_kafka_producer):
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.stop()
    assert not scouter_observer._running
    assert not scouter_observer._thread.is_alive()
