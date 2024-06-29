from typing import List
from scouter import DriftServerRecord


class BaseProducer:
    """Base class for all producers."""

    def publish(self, records: List[DriftServerRecord]) -> None:
        raise NotImplementedError

    def type(self) -> str:
        raise NotImplementedError
