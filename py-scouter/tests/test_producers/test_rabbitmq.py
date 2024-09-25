from scouter import RabbitMQConfig, RabbitMQProducer


def test_rabbit_config():
    config = RabbitMQConfig()

    print(config)

    assert config.connection_params.host == "localhost"
    assert config.raise_on_err


def _test_rabbit_producer(mock_rabbit_connection):
    config = RabbitMQConfig()

    producer = RabbitMQProducer(config)

    assert producer._rabbit_config == config
    assert producer.max_retries == 3
    assert producer._producer is not None
