import pytest
from unittest.mock import patch
from scouter import ScouterObserver, ObservabilityMetrics
import time


@pytest.fixture
def scouter_observer():
    with patch("scouter.Observer") as MockObserver:
        mock_observer = MockObserver.return_value
        scouter_observer = ScouterObserver("repo", "name", "version")
        yield scouter_observer, mock_observer
        scouter_observer.stop()


def test_add_request(scouter_observer):
    scouter, mock_observer = scouter_observer
    scouter.add_request_metrics("route", 0.1, "OK", 200)
    assert not scouter._queue.empty()
    request = scouter._queue.get()
    assert request == ("route", 0.1, "OK", 200)


def test_process_queue(scouter_observer) -> None:
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.add_request_metrics("route", 0.1, "OK", 200)
    time.sleep(0.1)
    metrics: ObservabilityMetrics = scouter_observer._observer.collect_metrics()
    assert metrics.request_count == 1
    scouter_observer.stop()


@patch("time.time", side_effect=[time.time() + 40])
def test_collect_and_reset_metrics(mock_time, scouter_observer):
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.add_request_metrics("route", 0.1, "OK", 200)
    time.sleep(0.1)  # Give some time for the background thread to process the queue
    metrics: ObservabilityMetrics = scouter_observer._observer.collect_metrics()

    # should be reset
    assert metrics is None


def test_stop(scouter_observer):
    scouter_observer, mock_observer = scouter_observer
    scouter_observer.stop()
    assert not scouter_observer._running
    assert not scouter_observer._thread.is_alive()
