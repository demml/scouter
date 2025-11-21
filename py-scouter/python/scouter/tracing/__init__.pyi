# pylint: skip-file

"""Tracing utilities for Scouter using OpenTelemetry."""

import datetime
from types import TracebackType
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Optional,
    ParamSpec,
    TypeAlias,
    TypeVar,
    Union,
)

from ..transport import HttpConfig, KafkaConfig, RabbitMQConfig, RedisConfig
from ..types import CompressionType

P = ParamSpec("P")
R = TypeVar("R")
SerializedType: TypeAlias = Union[str, int, float, dict, list]

def get_function_type(func: Callable[..., Any]) -> FunctionType:
    """Determine the function type (sync, async, generator, async generator).

    Args:
        func (Callable[..., Any]):
            The function to analyze.
    Returns:
        FunctionType:
            The determined function type.
    """
    ...

class OtelProtocol:
    """Enumeration of protocols for HTTP exporting."""

    HttpBinary: "OtelProtocol"
    HttpJson: "OtelProtocol"

class SpanKind:
    """Enumeration of span kinds."""

    Internal: "SpanKind"
    Server: "SpanKind"
    Client: "SpanKind"
    Producer: "SpanKind"
    Consumer: "SpanKind"

class FunctionType:
    """Enumeration of function types."""

    Sync: "FunctionType"
    Async: "FunctionType"
    Generator: "FunctionType"
    AsyncGenerator: "FunctionType"

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
        kind: SpanKind = SpanKind.Internal,
        attributes: List[dict[str, str]] = [],
        baggage: List[dict[str, str]] = [],
        tags: List[dict[str, str]] = [],
        label: Optional[str] = None,
        parent_context_id: Optional[str] = None,
        max_length: int = 1000,
        capture_last_stream_item: bool = False,
        join_stream_items: bool = False,
        *args,
        **kwargs,
    ) -> Callable[[Callable[P, R]], Callable[P, R]]:
        """Decorator to trace function execution with OpenTelemetry spans.

        Args:
            name (Optional[str]):
                The name of the span. If None, defaults to the function name.
            kind (SpanKind):
                The kind of span (e.g., Internal, Server, Client).
            label (Optional[str]):
                An optional label for the span.
            attributes (Optional[dict[str, str]]):
                Optional attributes to set on the span.
            baggage (Optional[dict[str, str]]):
                Optional baggage items to attach to the span.
            tags (Optional[dict[str, str]]):
                Optional tags to set on the span.
            parent_context_id (Optional[str]):
                Optional parent span context ID.
            max_length (int):
                The maximum length for string inputs/outputs. Defaults to 1000.
            func_type (FunctionType):
                The type of function being decorated (Sync, Async, Generator, AsyncGenerator).
            capture_last_stream_item (bool):
                Whether to capture only the last item from a generator/async generator.
            join_stream_items (bool):
                Whether to join all items from a generator/async generator into a list.
        Returns:
            Callable[[Callable[P, R]], Callable[P, R]]:
                A decorator that wraps the function with tracing logic.
        """

        ...

    def _start_decorated_as_current_span(
        self,
        name: Optional[str],
        func: Callable[..., Any],
        func_args: tuple[Any, ...],
        kind: SpanKind = SpanKind.Internal,
        label: Optional[str] = None,
        attributes: Optional[dict[str, str]] = None,
        baggage: Optional[dict[str, str]] = None,
        tags: Optional[dict[str, str]] = None,
        parent_context_id: Optional[str] = None,
        max_length: int = 1000,
        func_type: FunctionType = FunctionType.Sync,
        func_kwargs: Optional[dict[str, Any]] = None,
    ) -> ActiveSpan:
        """Context manager to start a new span as the current span for decorated functions.

        Args:
            name (Optional[str]):
                The name of the span. If None, defaults to the function name.
            func (Callable[..., Any]):
                The function being decorated.
            func_args (tuple[Any, ...]):
                The positional arguments passed to the function.
            kind (SpanKind):
                The kind of span (e.g., Internal, Server, Client).
            label (Optional[str]):
                An optional label for the span.
            attributes (Optional[dict[str, str]]):
                Optional attributes to set on the span.
            baggage (Optional[dict[str, str]]):
                Optional baggage items to attach to the span.
            tags (Optional[dict[str, str]]):
                Optional tags to set on the span.
            parent_context_id (Optional[str]):
                Optional parent span context ID.
            max_length (int):
                The maximum length for string inputs/outputs. Defaults to 1000.
            func_type (FunctionType):
                The type of function being decorated (Sync, Async, Generator, AsyncGenerator).
            func_kwargs (Optional[dict[str, Any]]):
                The keyword arguments passed to the function.
        Returns:
            ActiveSpan:
                The active span context manager.
        """
        ...

    @property
    def current_span(self) -> ActiveSpan:
        """Get the current active span, if any.
        This will return an Error if no span is active.

        Returns:
            ActiveSpan:
                The current active span.
        """
        ...

def get_tracer(name: str) -> Tracer:
    """Get a Tracer instance by name.

    Args:
        name (str):
            The name of the tracer/service.
    """
    ...

class BatchConfig:
    """Configuration for batch exporting of spans."""

    def __init__(
        self,
        max_queue_size: int = 2048,
        scheduled_delay_ms: int = 5000,
        max_export_batch_size: int = 512,
    ) -> None:
        """Initialize the BatchConfig.

        Args:
            max_queue_size (int):
                The maximum queue size for spans. Defaults to 2048.
            scheduled_delay_ms (int):
                The delay in milliseconds between export attempts. Defaults to 5000.
            max_export_batch_size (int):
                The maximum batch size for exporting spans. Defaults to 512.
        """
        ...

def init_tracer(
    service_name: str = "scouter_service",
    transport_config: HttpConfig
    | KafkaConfig
    | RabbitMQConfig
    | RedisConfig = HttpConfig(),
    exporter: HttpSpanExporter
    | StdoutSpanExporter
    | TestSpanExporter = StdoutSpanExporter(),  # noqa: F821
    batch_config: Optional[BatchConfig] = None,
    profile_space: Optional[str] = None,
    profile_name: Optional[str] = None,
    profile_version: Optional[str] = None,
) -> None:
    """Initialize the tracer for a service with specific transport and exporter configurations.

    This function configures a service tracer, allowing for the specification of
    the service name, the transport mechanism for exporting spans, and the chosen
    span exporter.

    Args:
        service_name (str):
            The **required** name of the service this tracer is associated with.
            This is typically a logical identifier for the application or component.
        transport_config (HttpConfig | KafkaConfig | RabbitMQConfig | RedisConfig | None):
            The configuration detailing how spans should be sent out.
            If **None**, a default `HttpConfig` will be used.

            The supported configuration types are:
            * `HttpConfig`: Configuration for exporting via HTTP/gRPC.
            * `KafkaConfig`: Configuration for exporting to a Kafka topic.
            * `RabbitMQConfig`: Configuration for exporting to a RabbitMQ queue.
            * `RedisConfig`: Configuration for exporting to a Redis stream or channel.
        exporter (HttpSpanExporter | StdoutSpanExporter | TestSpanExporter | None):
            The span exporter implementation to use.
            If **None**, a default `StdoutSpanExporter` is used.

            Available exporters:
            * `HttpSpanExporter`: Sends spans to an HTTP endpoint (e.g., an OpenTelemetry collector).
            * `StdoutSpanExporter`: Writes spans directly to standard output for debugging.
            * `TestSpanExporter`: Collects spans in memory, primarily for unit testing.
        batch_config (BatchConfig | None):
            Configuration for the batching process. If provided, spans will be queued
            and exported in batches according to these settings. If `None`, and the
            exporter supports batching, default batch settings will be applied.

    Drift Profile Association (Optional):
        Use these parameters to associate the tracer with a specific drift profile.

        profile_space (str | None):
            The space for the drift profile.
        profile_name (str | None):
            A name of the associated drift profile or service.
        profile_version (str | None):
            The version of the drift profile.
    """

class ActiveSpan:
    """Represents an active tracing span."""

    @property
    def context_id(self) -> str:
        """Get the context ID of the active span."""
        ...

    def set_attribute(self, key: str, value: SerializedType) -> None:
        """Set an attribute on the active span.

        Args:
            key (str):
                The attribute key.
            value (SerializedType):
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

    def __enter__(self) -> "ActiveSpan":
        """Enter the span context."""
        ...

    def __exit__(
        self,
        exc_type: Optional[type],
        exc_value: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None:
        """Exit the span context."""
        ...

    async def __aenter__(self) -> "ActiveSpan":
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
        kind: Optional[SpanKind] = SpanKind.Internal,
        label: Optional[str] = None,
        attributes: Optional[dict[str, str]] = None,
        baggage: Optional[dict[str, str]] = None,
        tags: Optional[dict[str, str]] = None,
        parent_context_id: Optional[str] = None,
    ) -> ActiveSpan:
        """Context manager to start a new span as the current span.

        Args:
            name (str):
                The name of the span.
            kind (Optional[SpanKind]):
                The kind of span (e.g., "SERVER", "CLIENT").
            label (Optional[str]):
                An optional label for the span.
            attributes (Optional[dict[str, str]]):
                Optional attributes to set on the span.
            baggage (Optional[dict[str, str]]):
                Optional baggage items to attach to the span.
            tags (Optional[dict[str, str]]):
                Optional tags to set on the span and trace.
            parent_context_id (Optional[str]):
                Optional parent span context ID.
        Returns:
            ActiveSpan:
        """
        ...

class StdoutSpanExporter:
    """Exporter that outputs spans to standard output (stdout)."""

    def __init__(
        self,
        batch_export: bool = False,
        sample_ratio: Optional[float] = None,
    ) -> None:
        """Initialize the StdoutSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to False.
            sample_ratio (Optional[float]):
                The sampling ratio for traces. If None, defaults to always sample.
        """

    @property
    def batch_export(self) -> bool:
        """Get whether batch exporting is enabled."""
        ...

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""
        ...

def flush_tracer() -> None:
    """Force flush the tracer's exporter."""
    ...

class ExportConfig:
    """Configuration for exporting spans."""

    def __init__(
        self,
        endpoint: Optional[str],
        protocol: OtelProtocol = OtelProtocol.HttpBinary,
        timeout: Optional[int] = None,
    ) -> None:
        """Initialize the ExportConfig.

        Args:
            endpoint (Optional[str]):
                The HTTP endpoint for exporting spans.
            protocol (Protocol):
                The protocol to use for exporting spans. Defaults to HttpBinary.
            timeout (Optional[int]):
                The timeout for HTTP requests in seconds.
        """

        ...

    @property
    def endpoint(self) -> Optional[str]:
        """Get the HTTP endpoint for exporting spans."""
        ...

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""
        ...

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for HTTP requests in seconds."""
        ...

class OtelHttpConfig:
    """Configuration for HTTP exporting."""

    def __init__(
        self,
        headers: Optional[dict[str, str]] = None,
        compression: Optional[CompressionType] = None,
    ) -> None:
        """Initialize the OtelHttpConfig.

        Args:
            headers (Optional[dict[str, str]]):
                Optional HTTP headers to include in requests.
            compression (Optional[CompressionType]):
                Optional compression type for HTTP requests.
        """

    @property
    def headers(self) -> Optional[dict[str, str]]:
        """Get the HTTP headers."""
        ...

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type."""
        ...

class HttpSpanExporter:
    """Exporter that sends spans to an HTTP endpoint."""

    def __init__(
        self,
        batch_export: bool = True,
        export_config: Optional[ExportConfig] = None,
        http_config: Optional[OtelHttpConfig] = None,
        sample_ratio: Optional[float] = None,
    ) -> None:
        """Initialize the HttpSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to True.
            export_config (Optional[ExportConfig]):
                Configuration for exporting spans.
            http_config (Optional[OtelHttpConfig]):
                Configuration for the HTTP exporter.
            sample_ratio (Optional[float]):
                The sampling ratio for traces. If None, defaults to always sample.
        """

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""
        ...

    @property
    def batch_export(self) -> bool:
        """Get whether batch exporting is enabled."""
        ...

    @property
    def endpoint(self) -> Optional[str]:
        """Get the HTTP endpoint for exporting spans."""
        ...

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""
        ...

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for HTTP requests in seconds."""
        ...

    @property
    def headers(self) -> Optional[dict[str, str]]:
        """Get the HTTP headers used for exporting spans."""
        ...

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type used for exporting spans."""
        ...

class GrpcConfig:
    """Configuration for gRPC exporting."""

    def __init__(self, compression: Optional[CompressionType] = None) -> None:
        """Initialize the GrpcConfig.

        Args:
            compression (Optional[CompressionType]):
                Optional compression type for gRPC requests.
        """

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type."""
        ...

class GrpcSpanExporter:
    """Exporter that sends spans to a gRPC endpoint."""

    def __init__(
        self,
        batch_export: bool = True,
        export_config: Optional[ExportConfig] = None,
        grpc_config: Optional[GrpcConfig] = None,
        sample_ratio: Optional[float] = None,
    ) -> None:
        """Initialize the GrpcSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to True.
            export_config (Optional[ExportConfig]):
                Configuration for exporting spans.
            grpc_config (Optional[GrpcConfig]):
                Configuration for the gRPC exporter.
            sample_ratio (Optional[float]):
                The sampling ratio for traces. If None, defaults to always sample.
        """

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""
        ...

    @property
    def batch_export(self) -> bool:
        """Get whether batch exporting is enabled."""
        ...

    @property
    def endpoint(self) -> Optional[str]:
        """Get the gRPC endpoint for exporting spans."""
        ...

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""
        ...

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for gRPC requests in seconds."""
        ...

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type used for exporting spans."""
        ...

class TraceRecord:
    created_at: datetime.datetime
    trace_id: str
    space: str
    name: str
    version: str
    scope: str
    trace_state: str
    start_time: datetime.datetime
    end_time: datetime.datetime
    duration_ms: int
    status: str
    root_span_id: str
    attributes: Optional[dict]

    def get_attributes(self) -> Dict[str, Any]: ...

class TraceSpanRecord:
    created_at: datetime.datetime
    span_id: str
    trace_id: str
    parent_span_id: Optional[str]
    space: str
    name: str
    version: str
    scope: str
    span_name: str
    span_kind: str
    start_time: datetime.datetime
    end_time: datetime.datetime
    duration_ms: int
    status_code: str
    status_message: str
    attributes: dict
    events: dict
    links: dict

    def get_attributes(self) -> Dict[str, Any]: ...
    def get_events(self) -> Dict[str, Any]: ...
    def get_links(self) -> Dict[str, Any]: ...
    def __str__(self) -> str: ...

class TraceBaggageRecord:
    created_at: datetime.datetime
    trace_id: str
    scope: str
    key: str
    value: str
    space: str
    name: str
    version: str

class TestSpanExporter:
    """Exporter for testing that collects spans in memory."""

    def __init__(self, batch_export: bool = True) -> None:
        """Initialize the TestSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to True.
        """

    @property
    def traces(self) -> list[TraceRecord]:
        """Get the collected trace records."""
        ...

    @property
    def spans(self) -> list[TraceSpanRecord]:
        """Get the collected trace span records."""
        ...

    @property
    def baggage(self) -> list[TraceBaggageRecord]:
        """Get the collected trace baggage records."""
        ...

    def clear(self) -> None:
        """Clear all collected trace records."""
        ...

def shutdown_tracer() -> None:
    """Shutdown the tracer and flush any remaining spans."""
    ...
