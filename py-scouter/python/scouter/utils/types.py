# pylint: disable=invalid-name

from enum import Enum


class AlertZone(str, Enum):
    Zone1 = "Zone 1"
    Zone2 = "Zone 2"
    Zone3 = "Zone 3"
    OutOfBounds = "Out of Bounds"
    NotApplicable = "NA"


class AlertType(str, Enum):
    OutOfBounds = "Out of Bounds"
    Consecutive = "Consecutive"
    Alternating = "Alternating"
    AllGood = "All Good"
    Trend = "Trend"


class ProducerTypes(str, Enum):
    Kafka = "Kafka"
    Http = "http"
