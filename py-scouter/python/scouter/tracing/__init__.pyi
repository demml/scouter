# pylint: skip-file

"""Tracing utilities for Scouter using OpenTelemetry."""

from types import TracebackType
from typing import Any, Callable, Optional, TypeVar, ParamSpec

P = ParamSpec("P")
R = TypeVar("R")

def get_current_span() -> Optional[ActiveSpan]:
    """
    Get the currently active span.

    This is a helper function to retrieve the currently active span when using the
    tracing decorator.

    Returns:
        The currently active ActiveSpan, or None if no span is active.

    Example:
        >>> @tracer.span("my_operation")
        ... def my_function():
        ...     span = get_current_span()
        ...     if span:
        ...         span.set_attribute("custom_key", "custom_value")
        ...         span.add_event("custom_event", {"detail": "some detail"})
    """
    ...

def set_current_span(span: Optional[ActiveSpan]) -> None:
    """Set the current active span (internal use)."""
    ...

class Tracer(BaseTracer):
    """
    Extended tracer with decorator support.

    This class extends the Rust BaseTracer to provide Python-friendly
    decorator functionality for tracing spans.

    Examples:
        >>> from scouter.tracing import init_tracer, get_tracer
        >>> init_tracer(name="my-service")
        >>> tracer = get_tracer("my-service")
        >>>
        >>> @tracer.span("operation_name")
        ... def my_function():
        ...     return "result"
    """

    def span(
        self,
        name: Optional[str] = None,
        *,
        kind: Optional[str] = None,
        attributes: Optional[dict[str, str]] = None,
        baggage: Optional[dict[str, str]] = None,
    ) -> Callable[[Callable[P, R]], Callable[P, R]]:
        """Decorator to trace function execution with OpenTelemetry spans."""

        ...

def get_tracer(name: str) -> Tracer:
    """Get a Tracer instance by name.

    Args:
        name (str):
            The name of the tracer/service.
    """
    ...

def init_tracer(
    name: Optional[str] = None,
    endpoint: Optional[str] = None,
    sample_ratio: Optional[str] = None,
) -> None:
    """Initialize the tracer with the given service name.

    Args:
        name (Optional[str]):
            The name of the service for tracing.
        endpoint (Optional[str]):
            The endpoint for exporting traces.
        sample_ratio (Optional[str]):
            The sampling ratio for traces.
    """

class ActiveSpan:
    """Represents an active tracing span."""

    @property
    def context_id(self) -> str:
        """Get the context ID of the active span."""
        ...

    def set_attribute(self, key: str, value: str) -> None:
        """Set an attribute on the active span.

        Args:
            key (str):
                The attribute key.
            value (str):
                The attribute value.
        """
        ...

    def add_event(self, name: str, attributes: Any) -> None:
        """Add an event to the active span.

        Args:
            name (str):
                The name of the event.
            attributes (Any):
                Optional attributes for the event.
                Can be any serializable type or pydantic `BaseModel`.
        """
        ...

    def set_status(self, status: str, description: Optional[str] = None) -> None:
        """Set the status of the active span.

        Args:
            status (str):
                The status code (e.g., "OK", "ERROR").
            description (Optional[str]):
                Optional description for the status.
        """
        ...

    def set_input(self, input: Any, max_length: int = 1000) -> None:
        """Set the input for the active span.

        Args:
            input (Any):
                The input to set. Can be any serializable primitive type (str, int, float, bool, list, dict),
                or a pydantic `BaseModel`.
            max_length (int):
                The maximum length for a given string input. Defaults to 1000.
        """
        ...

    def set_output(self, output: Any, max_length: int = 1000) -> None:
        """Set the output for the active span.

        Args:
            output (Any):
                The output to set. Can be any serializable primitive type (str, int, float, bool, list, dict),
                or a pydantic `BaseModel`.
            max_length (int):
                The maximum length for a given string output. Defaults to 1000.

        """
        ...

    def __exit__(
        self,
        exc_type: Optional[type],
        exc_value: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None:
        """Exit the span context."""
        ...

    def __aenter__(self) -> "ActiveSpan":
        """Enter the async span context."""
        ...

    async def __aexit__(
        self,
        exc_type: Optional[type],
        exc_value: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None:
        """Exit the async span context."""
        ...

class BaseTracer:
    def __init__(self, name: str) -> None:
        """Initialize the BaseTracer with a service name.

        Args:
            name (str):
                The name of the service for tracing.
        """

    def start_as_current_span(
        self,
        name: str,
        *,
        kind: Optional[str] = None,
        attributes: Optional[dict[str, str]] = None,
        baggage: Optional[dict[str, str]] = None,
        parent_context_id: Optional[str] = None,
    ) -> ActiveSpan:
        """Context manager to start a new span as the current span.

        Args:
            name (str):
                The name of the span.
            kind (Optional[str]):
                The kind of span (e.g., "SERVER", "CLIENT").
            attributes (Optional[dict[str, str]]):
                Optional attributes to set on the span.
            baggage (Optional[dict[str, str]]):
                Optional baggage items to attach to the span.
            parent_context_id (Optional[str]):
                Optional parent span context ID.
        Returns:
            ActiveSpan:
        """
        ...
