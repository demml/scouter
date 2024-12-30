import pytest
from scouter import DriftRecordProducer, HTTPConfig, KafkaConfig
from scouter.utils.types import ProducerTypes


def test_http_producer(mock_httpx_producer):
    http_config = HTTPConfig(
        server_url="http://localhost:8000",
        username="test-username",
        password="test-password",
    )
    producer = DriftRecordProducer.get_producer(config=http_config)
    assert producer.type() == "http"


def test_kafka_producer(mock_kafka_producer):
    kafka_config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
    )
    producer = DriftRecordProducer.get_producer(config=kafka_config)
    assert producer.type() == ProducerTypes.Kafka


def test_fail_production():
    with pytest.raises(ValueError):
        DriftRecordProducer.get_producer(config="test")
