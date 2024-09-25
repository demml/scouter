from typing import Union

from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig, HTTPProducer
from scouter.integrations.kafka import KafkaConfig, KafkaProducer
from scouter.integrations.rabbitmq import RabbitMQConfig, RabbitMQProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes

logger = ScouterLogger.get_logger()


class DriftRecordProducer:
    """Helper class to get the producer based on the producer type"""

    @staticmethod
    def get_producer(
        config: Union[HTTPConfig, KafkaConfig, RabbitMQConfig],
    ) -> BaseProducer:
        """Gets the producer based on the producer type

        Args:
            config:
                Configuration for the producer

        Returns:
            BaseProducer: Producer instance
        """
        if not isinstance(config, (HTTPConfig, KafkaConfig, RabbitMQConfig)):
            raise ValueError(
                f"""config must be an instance of either HTTPConfig, KafkaConfig or RabbitMQConfig.
                Received {type(config)}"""
            )

        if config.type == ProducerTypes.Http:
            assert isinstance(config, HTTPConfig)
            return HTTPProducer(config)

        if config.type == ProducerTypes.RabbitMQ:
            assert isinstance(config, RabbitMQConfig)
            return RabbitMQProducer(config)

        assert isinstance(config, KafkaConfig)
        return KafkaProducer(config)
