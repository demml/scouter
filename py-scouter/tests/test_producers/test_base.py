from scouter.integrations.base import BaseProducer


def test_base_producer():
    producer = BaseProducer()
    try:
        producer.publish(None)
    except NotImplementedError:
        assert True
    else:
        assert False

    try:
        producer.flush()
    except NotImplementedError:
        assert True
    else:
        assert False

    try:
        producer.type()
    except NotImplementedError:
        assert True
    else:
        assert False
