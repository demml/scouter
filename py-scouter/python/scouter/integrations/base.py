from typing import Optional, Union

from .._scouter import ServerRecords


class BaseProducer:
    """Base class for all producers."""

    def publish(self, records: Union[ServerRecords]) -> None:
        raise NotImplementedError

    def flush(self, timeout: Optional[float] = None) -> None:
        raise NotImplementedError

    @staticmethod
    def type() -> str:
        raise NotImplementedError
