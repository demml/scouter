from pika import BasicProperties, ConnectionParameters  # type: ignore
from scouter import (
    CustomMetricServerRecord,
    PsiServerRecord,
    RabbitMQConfig,
    RabbitMQProducer,
    RecordType,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


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


def test_rabbit_producer_spc(mock_rabbit_connection):
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(connection_params=params)

    producer = RabbitMQProducer(config)

    assert producer._rabbit_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None

    record = SpcServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(
        ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )
    producer.flush()
    producer.flush(10)


def test_rabbit_producer_psi(mock_rabbit_connection):
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(connection_params=params)

    producer = RabbitMQProducer(config)

    assert producer._rabbit_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None

    record = PsiServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        feature="test",
        bin_id="test",
        bin_count=1,
    )

    producer.publish(
        ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )
    producer.flush()
    producer.flush(10)


def test_rabbit_producer_custom(mock_rabbit_connection):
    params = ConnectionParameters(host="localhost")
    config = RabbitMQConfig(connection_params=params)

    producer = RabbitMQProducer(config)

    assert producer._rabbit_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None

    record = CustomMetricServerRecord(
        name="test",
        repository="test",
        version="1.0.0",
        metric="metric",
        value=0.1,
    )

    producer.publish(
        ServerRecords(
            records=[ServerRecord(record)],
            record_type=RecordType.Spc,
        )
    )
    producer.flush()
    producer.flush(10)
