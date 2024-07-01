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
    def get_producer(producer_type: ProducerTypes, config: Union[HTTPConfig, KafkaConfig]) -> BaseProducer:
        """Gets the producer based on the producer type

        Args:
            producer_type:
                Type of producer to get

            config:
                Configuration for the producer

        Returns:
            BaseProducer: Producer instance
        """
        if producer_type == ProducerTypes.Http:
            assert isinstance(config, HTTPConfig)
            return HTTPProducer(config)

        elif producer_type == ProducerTypes.Kafka:
            assert isinstance(config, KafkaConfig)
            return KafkaProducer(config)
        else:
            raise ValueError(f"Producer type {producer_type} not supported")
