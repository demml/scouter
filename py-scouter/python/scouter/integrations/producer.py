from typing import Union

from scouter.integrations.base import BaseProducer
from scouter.integrations.http import HTTPConfig, HTTPProducer
from scouter.integrations.kafka import KafkaConfig, KafkaProducer
from scouter.utils.logger import ScouterLogger
from scouter.utils.types import ProducerTypes

logger = ScouterLogger.get_logger()


class DriftRecordProducer:
    """Helper class to get the producer based on the producer type"""

    @staticmethod
    def get_producer(config: Union[HTTPConfig, KafkaConfig]) -> BaseProducer:
        """Gets the producer based on the producer type

        Args:
            config:
                Configuration for the producer

        Returns:
            BaseProducer: Producer instance
        """
        if not isinstance(config, (HTTPConfig, KafkaConfig)):
            raise ValueError(f"config must be an instance of either HTTPConfig or KafkaConfig, got {type(config)}")

        if config.type == ProducerTypes.Http:
            assert isinstance(config, HTTPConfig)
            return HTTPProducer(config)

        assert isinstance(config, KafkaConfig)
        return KafkaProducer(config)
