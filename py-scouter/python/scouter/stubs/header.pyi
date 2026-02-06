# pylint: disable=redefined-builtin, invalid-name, dangerous-default-value, missing-final-newline

import datetime
from pathlib import Path
from types import TracebackType
from typing import (
    TYPE_CHECKING,
    Any,
    Callable,
    Dict,
    Generic,
    List,
    Literal,
    Optional,
    ParamSpec,
    Protocol,
    Sequence,
    Tuple,
    Type,
    TypeAlias,
    Union,
    overload,
)

from typing_extensions import TypeVar

if TYPE_CHECKING:
    from opentelemetry.sdk.trace.export import SpanExporter as _OtelSpanExporter
    from opentelemetry.sdk.trace.export import SpanExportResult as _OtelSpanExportResult

    _SpanExporterBase = _OtelSpanExporter
    _SpanExportResult = _OtelSpanExportResult
else:
    # Runtime fallback - anything with these methods works
    class _SpanExporterBase:
        def export(self, spans: Any) -> Any: ...
        def shutdown(self) -> None: ...
        def force_flush(self, timeout_millis: int = 30000) -> bool: ...

    class _SpanExportResult:
        SUCCESS: 0
        FAILURE: 1

SerializedType: TypeAlias = Union[str, int, float, dict, list]
Context: TypeAlias = Union[Dict[str, Any], "BaseModel"]

P = ParamSpec("P")
R = TypeVar("R")

class BaseModel(Protocol):
    """Protocol for pydantic BaseModel to ensure compatibility with context"""

    def model_dump(self) -> Dict[str, Any]:
        """Dump the model as a dictionary"""

    def model_dump_json(self) -> str:
        """Dump the model as a JSON string"""

    def __str__(self) -> str:
        """String representation of the model"""
