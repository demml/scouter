from scouter.queue import (
    CustomMetricServerRecord,
    PsiServerRecord,
    RabbitMQConfig,
    ScouterProducer,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)


def test_rabbit_config():
    config = RabbitMQConfig(raise_on_error=True)
    assert config.raise_on_error


def test_rabbit_producer_spc(rabbitmq_scouter_server):
    config = RabbitMQConfig()

    producer = ScouterProducer(config)

    record = SpcServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        feature="test",
        value=0.1,
    )

    producer.publish(ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_rabbit_producer_psi(rabbitmq_scouter_server):
    config = RabbitMQConfig()
    assert config.max_retries == 3

    producer = ScouterProducer(config)

    record = PsiServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        feature="test",
        bin_id=0,
        bin_count=1,
    )

    producer.publish(ServerRecords(records=[ServerRecord(record)]))
    producer.flush()


def test_rabbit_producer_custom(rabbitmq_scouter_server):
    config = RabbitMQConfig()
    producer = ScouterProducer(config)

    record = CustomMetricServerRecord(
        name="test",
        space="test",
        version="1.0.0",
        metric="metric",
        value=0.1,
    )

    producer.publish(
        ServerRecords(
            records=[ServerRecord(record)],
        )
    )
    producer.flush()
