from scouter import (
    CustomMetricServerRecord,
    PsiServerRecord,
    RecordType,
    ServerRecord,
    ServerRecords,
    SpcServerRecord,
)
from scouter.queue import RabbitMQConfig, ScouterProducer


def test_rabbit_config():
    config = RabbitMQConfig(raise_on_error=True)
    assert config.raise_on_error


def test_rabbit_producer_spc():
    config = RabbitMQConfig()

    producer = ScouterProducer(config)

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


def test_rabbit_producer_psi():
    config = RabbitMQConfig()
    assert config.max_retries == 3

    producer = ScouterProducer(config)

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


def test_rabbit_producer_custom():
    config = RabbitMQConfig()
    producer = ScouterProducer(config)

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
