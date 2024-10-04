import pytest
from unittest.mock import patch
from scouter import ScouterObserver, ObservabilityMetrics
import time


@pytest.fixture
@patch("scouter.integrations.observer.Observer")
def scouter_observer(MockObserver):
    mock_observer = MockObserver.return_value
    scouter_observer = ScouterObserver("repo", "name", "version")
    yield scouter_observer, mock_observer
    scouter_observer.stop()


def test_add_request(scouter_observer):
    scouter, mock_observer = scouter_observer
    scouter.add_request(("route", 0.1, "OK", 200))
    assert not scouter._queue.empty()
    request = scouter._queue.get()
    assert request == ("route", 0.1, "OK", 200)


def test_process_queue(scouter_observer):
    scouter, mock_observer = scouter_observer
    scouter.add_request(("route", 0.1, "OK", 200))
    time.sleep(2)  # Give some time for the background thread to process the queue
    mock_observer.increment.assert_called_with("route", 0.1, "OK", 200)


@patch("time.time", side_effect=[0, 31, 32])
def test_collect_and_reset_metrics(mock_time, scouter_observer):
    scouter, mock_observer = scouter_observer
    mock_observer.collect_metrics.return_value = ObservabilityMetrics(request_count=10)
    time.sleep(2)  # Give some time for the background thread to process the queue
    mock_observer.collect_metrics.assert_called_once()
    mock_observer.reset_metrics.assert_called_once()


def test_stop(scouter_observer):
    scouter, mock_observer = scouter_observer
    scouter.stop()
    assert not scouter._running
    assert not scouter._thread.is_alive()
