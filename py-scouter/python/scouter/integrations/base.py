from typing import Optional, List

from .._scouter import DriftServerRecord


class BaseProducer:
    """Base class for all producers."""

    def publish(self, record: List[DriftServerRecord]) -> None:
        raise NotImplementedError

    def flush(self, timeout: Optional[float] = None) -> None:
        raise NotImplementedError

    @staticmethod
    def type() -> str:
        raise NotImplementedError
