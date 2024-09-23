from typing import Optional

from .._scouter import DriftServerRecords


class BaseProducer:
    """Base class for all producers."""

    def publish(self, records: DriftServerRecords) -> None:
        raise NotImplementedError

    def flush(self, timeout: Optional[float] = None) -> None:
        raise NotImplementedError

    @staticmethod
    def type() -> str:
        raise NotImplementedError
