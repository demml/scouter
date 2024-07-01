from scouter import KafkaConfig, KafkaProducer


def test_kafka_config():
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_err
    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": 600_000,
        "message.max.bytes": 2097164,
    }


def test_kafka_config_credentials(monkeypatch):
    monkeypatch.setenv("KAFKA_SASL_USERNAME", "test-username")
    monkeypatch.setenv("KAFKA_SASL_PASSWORD", "test-password")

    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    assert config.topic == "test-topic"
    assert config.brokers == "localhost:9092"
    assert config.raise_on_err
    assert config.config == {
        "bootstrap.servers": "localhost:9092",
        "compression.type": "gzip",
        "message.timeout.ms": 600_000,
        "message.max.bytes": 2097164,
        "sasl.mechanisms": "PLAIN",
        "sasl.password": "test-password",
        "sasl.username": "test-username",
        "security.protocol": "SASL_SSL",
    }
