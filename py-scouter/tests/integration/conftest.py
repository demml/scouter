import pytest
from scouter.mock import ScouterTestServer


@pytest.fixture()
def http_scouter_server():
    with ScouterTestServer() as server:
        yield server


@pytest.fixture()
def kafka_scouter_server():
    with ScouterTestServer(kafka=True) as server:
        yield server


@pytest.fixture()
def rabbitmq_scouter_server():
    with ScouterTestServer(rabbit_mq=True) as server:
        yield server
