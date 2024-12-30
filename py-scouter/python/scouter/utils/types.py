# pylint: disable=invalid-name

from enum import Enum


class ProducerTypes(str, Enum):
    Kafka = "Kafka"
    RabbitMQ = "RabbitMQ"
    Http = "http"


class DataType(str, Enum):
    FLOAT32 = "float32"
    FLOAT64 = "float64"
    INT8 = "int8"
    INT16 = "int16"
    INT32 = "int32"
    INT64 = "int64"

    @staticmethod
    def str_to_bits(dtype: str) -> str:
        bits = {
            "float32": "32",
            "float64": "64",
        }
        return bits[dtype]


class Constants(str, Enum):
    MISSING = "__missing__"
