from scouter import (
    RabbitMQConfig,
    RabbitMQProducer,
    SpcDriftServerRecord,
    SpcDriftServerRecords,
)
from pika import ConnectionParameters, BasicProperties  # type: ignore


def test_rabbit_config():
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(connection_params=params)

    assert config.connection_params.host == "localhost"
    assert config.raise_on_err


def test_rabbit_config_publish_properties():
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(
        connection_params=params,
        publish_properties=BasicProperties(),
    )

    assert config.connection_params.host == "localhost"
    assert config.raise_on_err


def test_rabbit_producer(mock_rabbit_connection):
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(connection_params=params)

    producer = RabbitMQProducer(config)

    assert producer._rabbit_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None

    record = SpcDriftServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(SpcDriftServerRecords(records=[record]))
    producer.flush()
    producer.flush(10)
