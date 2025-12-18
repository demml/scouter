# pylint: disable=dangerous-default-value,redefined-builtin,missing-param-doc
# python/scouter/_scouter.pyi
from datetime import datetime, timedelta
from pathlib import Path
from types import TracebackType
from typing import (
    Any,
    Callable,
    Dict,
    List,
    Literal,
    Optional,
    ParamSpec,
    Protocol,
    Sequence,
    TypeAlias,
    TypeVar,
    Union,
    overload,
)

SerializedType: TypeAlias = Union[str, int, float, dict, list]
Context: TypeAlias = Union[Dict[str, Any], "BaseModel"]

P = ParamSpec("P")
R = TypeVar("R")
########################
# __scouter.tracing____
########################

def get_function_type(func: Callable[..., Any]) -> "FunctionType":
    """Determine the function type (sync, async, generator, async generator).

    Args:
        func (Callable[..., Any]):
            The function to analyze.
    """

def get_tracing_headers_from_current_span() -> Dict[str, str]:
    """Get tracing headers from the current active span and global propagator.

    Returns:
        Dict[str, str]:
            A dictionary of tracing headers.
    """

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
    SyncGenerator: "FunctionType"
    AsyncGenerator: "FunctionType"

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

def init_tracer(
    service_name: str = "scouter_service",
    scope: str = "scouter.tracer.{version}",
    transport_config: Optional[HttpConfig | KafkaConfig | RabbitMQConfig | RedisConfig | GrpcConfig] = None,
    exporter: Optional[HttpSpanExporter | GrpcSpanExporter | StdoutSpanExporter | TestSpanExporter] = None,
    batch_config: Optional[BatchConfig] = None,
) -> None:
    """
    Initialize the tracer for a service with dual export capability.
    ```
    ╔════════════════════════════════════════════╗
    ║          DUAL EXPORT ARCHITECTURE          ║
    ╠════════════════════════════════════════════╣
    ║                                            ║
    ║  Your Application                          ║
    ║       │                                    ║
    ║       │  init_tracer()                     ║
    ║       │                                    ║
    ║       ├──────────────────┬                 ║
    ║       │                  │                 ║
    ║       ▼                  ▼                 ║
    ║  ┌─────────────┐   ┌──────────────┐        ║
    ║  │  Transport  │   │   Optional   │        ║
    ║  │   to        │   │     OTEL     │        ║
    ║  │  Scouter    │   │  Exporter    │        ║
    ║  │  (Required) │   │              │        ║
    ║  └──────┬──────┘   └──────┬───────┘        ║
    ║         │                 │                ║
    ║         │                 │                ║
    ║    ┌────▼────┐       ┌────▼────┐           ║
    ║    │ Scouter │       │  OTEL   │           ║
    ║    │ Server  │       │Collector│           ║
    ║    └─────────┘       └─────────┘           ║
    ║                                            ║
    ╚════════════════════════════════════════════╝
    ```
    Configuration Overview:
        This function sets up a service tracer with **mandatory** export to Scouter
        and **optional** export to OpenTelemetry-compatible backends.

    ```
    ┌─ REQUIRED: Scouter Export ────────────────────────────────────────────────┐
    │                                                                           │
    │  All spans are ALWAYS exported to Scouter via transport_config:           │
    │    • HttpConfig    → HTTP endpoint (default)                              │
    │    • GrpcConfig    → gRPC endpoint                                        │
    │    • KafkaConfig   → Kafka topic                                          │
    │    • RabbitMQConfig→ RabbitMQ queue                                       │
    │    • RedisConfig   → Redis stream/channel                                 │
    │                                                                           │
    └───────────────────────────────────────────────────────────────────────────┘

    ┌─ OPTIONAL: OTEL Export ───────────────────────────────────────────────────┐
    │                                                                           │
    │  Optionally export spans to external OTEL-compatible systems:             │
    │    • HttpSpanExporter   → OTEL Collector (HTTP)                           │
    │    • GrpcSpanExporter   → OTEL Collector (gRPC)                           │
    │    • StdoutSpanExporter → Console output (debugging)                      │
    │    • TestSpanExporter   → In-memory (testing)                             │
    │                                                                           │
    │  If None: Only Scouter export is active (NoOpExporter)                    │
    │                                                                           │
    └───────────────────────────────────────────────────────────────────────────┘
    ```

    Args:
        service_name (str):
            The **required** name of the service this tracer is associated with.
            This is typically a logical identifier for the application or component.
            Default: "scouter_service"

        scope (str):
            The scope for the tracer. Used to differentiate tracers by version
            or environment.
            Default: "scouter.tracer.{version}"

        transport_config (HttpConfig | GrpcConfig | KafkaConfig | RabbitMQConfig | RedisConfig | None):

            Configuration for sending spans to Scouter. If None, defaults to HttpConfig.

            Supported transports:
                • HttpConfig     : Export to Scouter via HTTP
                • GrpcConfig     : Export to Scouter via gRPC
                • KafkaConfig    : Export to Scouter via Kafka
                • RabbitMQConfig : Export to Scouter via RabbitMQ
                • RedisConfig    : Export to Scouter via Redis

        exporter (HttpSpanExporter | GrpcSpanExporter | StdoutSpanExporter | TestSpanExporter | None):

            Optional secondary exporter for OpenTelemetry-compatible backends.
            If None, spans are ONLY sent to Scouter (NoOpExporter used internally).

            Available exporters:
                • HttpSpanExporter   : Send to OTEL Collector via HTTP
                • GrpcSpanExporter   : Send to OTEL Collector via gRPC
                • StdoutSpanExporter : Write to stdout (debugging)
                • TestSpanExporter   : Collect in-memory (testing)

        batch_config (BatchConfig | None):
            Configuration for batch span export. If provided, spans are queued
            and exported in batches. If None and the exporter supports batching,
            default batch settings apply.

            Batching improves performance for high-throughput applications.

    Examples:
        Basic setup (Scouter only via HTTP):
            >>> init_tracer(service_name="my-service")

        Scouter via Kafka + OTEL Collector:
            >>> init_tracer(
            ...     service_name="my-service",
            ...     transport_config=KafkaConfig(brokers="kafka:9092"),
            ...     exporter=HttpSpanExporter(
            ...         export_config=OtelExportConfig(
            ...             endpoint="http://otel-collector:4318"
            ...         )
            ...     )
            ... )

        Scouter via gRPC + stdout debugging:
            >>> init_tracer(
            ...     service_name="my-service",
            ...     transport_config=GrpcConfig(server_uri="grpc://scouter:50051"),
            ...     exporter=StdoutSpanExporter()
            ... )

    Notes:
        • Spans are ALWAYS exported to Scouter via transport_config
        • OTEL export via exporter is completely optional
        • Both exports happen in parallel without blocking each other
        • Use batch_config to optimize performance for high-volume tracing

    See Also:
        - HttpConfig, GrpcConfig, KafkaConfig, RabbitMQConfig, RedisConfig
        - HttpSpanExporter, GrpcSpanExporter, StdoutSpanExporter, TestSpanExporter
        - BatchConfig
    """

class ActiveSpan:
    """Represents an active tracing span."""

    @property
    def trace_id(self) -> str:
        """Get the trace ID of the current active span.

        Returns:
            str:
                The trace ID.
        """

    @property
    def span_id(self) -> str:
        """Get the span ID of the current active span.

        Returns:
            str:
                The span ID.
        """

    @property
    def context_id(self) -> str:
        """Get the context ID of the active span."""

    def set_attribute(self, key: str, value: SerializedType) -> None:
        """Set an attribute on the active span.

        Args:
            key (str):
                The attribute key.
            value (SerializedType):
                The attribute value.
        """

    def set_tag(self, key: str, value: str) -> None:
        """Set a tag on the active span. Tags are similar to attributes
        except they are often used for indexing and searching spans/traces.
        All tags are also set as attributes on the span. Before export, tags are
        extracted and stored in a separate backend table for efficient querying.

        Args:
            key (str):
                The tag key.
            value (str):
                The tag value.
        """

    def add_event(self, name: str, attributes: Any) -> None:
        """Add an event to the active span.

        Args:
            name (str):
                The name of the event.
            attributes (Any):
                Optional attributes for the event.
                Can be any serializable type or pydantic `BaseModel`.
        """

    def set_status(self, status: str, description: Optional[str] = None) -> None:
        """Set the status of the active span.

        Args:
            status (str):
                The status code (e.g., "OK", "ERROR").
            description (Optional[str]):
                Optional description for the status.
        """

    def set_input(self, input: Any, max_length: int = 1000) -> None:
        """Set the input for the active span.

        Args:
            input (Any):
                The input to set. Can be any serializable primitive type (str, int, float, bool, list, dict),
                or a pydantic `BaseModel`.
            max_length (int):
                The maximum length for a given string input. Defaults to 1000.
        """

    def set_output(self, output: Any, max_length: int = 1000) -> None:
        """Set the output for the active span.

        Args:
            output (Any):
                The output to set. Can be any serializable primitive type (str, int, float, bool, list, dict),
                or a pydantic `BaseModel`.
            max_length (int):
                The maximum length for a given string output. Defaults to 1000.

        """

    def __enter__(self) -> "ActiveSpan":
        """Enter the span context."""

    def __exit__(
        self,
        exc_type: Optional[type],
        exc_value: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None:
        """Exit the span context."""

    async def __aenter__(self) -> "ActiveSpan":
        """Enter the async span context."""

    async def __aexit__(
        self,
        exc_type: Optional[type],
        exc_value: Optional[BaseException],
        exc_tb: Optional[TracebackType],
    ) -> None:
        """Exit the async span context."""

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
        trace_id: Optional[str] = None,
        span_id: Optional[str] = None,
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
            trace_id (Optional[str]):
                Optional trace ID to associate with the span. This is useful for
                when linking spans across different services or systems.
            span_id (Optional[str]):
                Optional span ID to associate with the span. This will be the parent span ID.
        Returns:
            ActiveSpan:
        """

    def _start_decorated_as_current_span(
        self,
        name: Optional[str],
        func: Callable[..., Any],
        func_args: tuple[Any, ...],
        kind: SpanKind = SpanKind.Internal,
        label: Optional[str] = None,
        attributes: List[dict[str, str]] = [],
        baggage: List[dict[str, str]] = [],
        tags: List[dict[str, str]] = [],
        parent_context_id: Optional[str] = None,
        trace_id: Optional[str] = None,
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
            trace_id (Optional[str]):
                Optional trace ID to associate with the span. This is useful for
                when linking spans across different services or systems.
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

    def current_span(self) -> ActiveSpan:
        """Get the current active span.

        Returns:
            ActiveSpan:
                The current active span.
                Raises an error if no active span exists.
        """

def get_current_active_span(self) -> ActiveSpan:
    """Get the current active span.

    Returns:
        ActiveSpan:
            The current active span.
            Raises an error if no active span exists.
    """

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

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""

def flush_tracer() -> None:
    """Force flush the tracer's exporter."""

class OtelExportConfig:
    """Configuration for exporting spans."""

    def __init__(
        self,
        endpoint: Optional[str],
        protocol: OtelProtocol = OtelProtocol.HttpBinary,
        timeout: Optional[int] = None,
        compression: Optional[CompressionType] = None,
        headers: Optional[dict[str, str]] = None,
    ) -> None:
        """Initialize the ExportConfig.

        Args:
            endpoint (Optional[str]):
                The endpoint for exporting spans. Can be either an HTTP or gRPC endpoint.
            protocol (Protocol):
                The protocol to use for exporting spans. Defaults to HttpBinary.
            timeout (Optional[int]):
                The timeout for requests in seconds.
            compression (Optional[CompressionType]):
                The compression type for requests.
            headers (Optional[dict[str, str]]):
                Optional HTTP headers to include in requests.
        """

    @property
    def endpoint(self) -> Optional[str]:
        """Get the HTTP endpoint for exporting spans."""

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for requests in seconds."""

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type used for exporting spans."""

    @property
    def headers(self) -> Optional[dict[str, str]]:
        """Get the HTTP headers used for exporting spans."""

class HttpSpanExporter:
    """Exporter that sends spans to an HTTP endpoint."""

    def __init__(
        self,
        batch_export: bool = True,
        export_config: Optional[OtelExportConfig] = None,
        sample_ratio: Optional[float] = None,
    ) -> None:
        """Initialize the HttpSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to True.
            export_config (Optional[OtelExportConfig]):
                Configuration for exporting spans.
            sample_ratio (Optional[float]):
                The sampling ratio for traces. If None, defaults to always sample.
        """

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""

    @property
    def batch_export(self) -> bool:
        """Get whether batch exporting is enabled."""

    @property
    def endpoint(self) -> Optional[str]:
        """Get the HTTP endpoint for exporting spans."""

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for HTTP requests in seconds."""

    @property
    def headers(self) -> Optional[dict[str, str]]:
        """Get the HTTP headers used for exporting spans."""

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type used for exporting spans."""

class GrpcSpanExporter:
    """Exporter that sends spans to a gRPC endpoint."""

    def __init__(
        self,
        batch_export: bool = True,
        export_config: Optional[OtelExportConfig] = None,
        sample_ratio: Optional[float] = None,
    ) -> None:
        """Initialize the GrpcSpanExporter.

        Args:
            batch_export (bool):
                Whether to use batch exporting. Defaults to True.
            export_config (Optional[OtelExportConfig]):
                Configuration for exporting spans.
            sample_ratio (Optional[float]):
                The sampling ratio for traces. If None, defaults to always sample.
        """

    @property
    def sample_ratio(self) -> Optional[float]:
        """Get the sampling ratio."""

    @property
    def batch_export(self) -> bool:
        """Get whether batch exporting is enabled."""

    @property
    def endpoint(self) -> Optional[str]:
        """Get the gRPC endpoint for exporting spans."""

    @property
    def protocol(self) -> OtelProtocol:
        """Get the protocol used for exporting spans."""

    @property
    def timeout(self) -> Optional[int]:
        """Get the timeout for gRPC requests in seconds."""

    @property
    def compression(self) -> Optional[CompressionType]:
        """Get the compression type used for exporting spans."""

class TraceRecord:
    created_at: datetime
    trace_id: str
    space: str
    name: str
    version: str
    scope: str
    trace_state: str
    start_time: datetime
    end_time: datetime
    duration_ms: int
    status: str
    root_span_id: str
    attributes: Optional[dict]

    def get_attributes(self) -> Dict[str, Any]: ...

class TraceSpanRecord:
    created_at: datetime
    span_id: str
    trace_id: str
    parent_span_id: Optional[str]
    space: str
    name: str
    version: str
    scope: str
    span_name: str
    span_kind: str
    start_time: datetime
    end_time: datetime
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

    @property
    def spans(self) -> list[TraceSpanRecord]:
        """Get the collected trace span records."""

    @property
    def baggage(self) -> list[TraceBaggageRecord]:
        """Get the collected trace baggage records."""

    def clear(self) -> None:
        """Clear all collected trace records."""

def shutdown_tracer() -> None:
    """Shutdown the tracer and flush any remaining spans."""

#########################
# _scouter.genai types
#########################

class Modality:
    """Represents different modalities for content generation."""

    ModalityUnspecified: "Modality"
    Text: "Modality"
    Image: "Modality"
    Audio: "Modality"

class ThinkingConfig:
    """Configuration for thinking/reasoning capabilities."""

    def __init__(
        self,
        include_thoughts: Optional[bool] = None,
        thinking_budget: Optional[int] = None,
    ) -> None: ...

class MediaResolution:
    """Media resolution settings for content generation."""

    MediaResolutionUnspecified: "MediaResolution"
    MediaResolutionLow: "MediaResolution"
    MediaResolutionMedium: "MediaResolution"
    MediaResolutionHigh: "MediaResolution"

class SpeechConfig:
    """Configuration for speech generation."""

    def __init__(
        self,
        voice_config: Optional["VoiceConfig"] = None,
        language_code: Optional[str] = None,
    ) -> None: ...

class PrebuiltVoiceConfig:
    """Configuration for prebuilt voice models."""

    def __init__(
        self,
        voice_name: str,
    ) -> None: ...

class VoiceConfigMode:
    PrebuiltVoiceConfig: "VoiceConfigMode"

class VoiceConfig:
    """Configuration for voice generation."""

    def __init__(self, voice_config: VoiceConfigMode) -> None: ...

class GenerationConfig:
    """Configuration for content generation with comprehensive parameter control.

    This class provides fine-grained control over the generation process including
    sampling parameters, output format, modalities, and various specialized features.

    Examples:
        Basic usage with temperature control:

        ```python
        GenerationConfig(temperature=0.7, max_output_tokens=1000)
        ```

        Multi-modal configuration:
        ```python
        config = GenerationConfig(
            response_modalities=[Modality.TEXT, Modality.AUDIO],
            speech_config=SpeechConfig(language_code="en-US")
        )
        ```

        Advanced sampling with penalties:
        ```python
        config = GenerationConfig(
            temperature=0.8,
            top_p=0.9,
            top_k=40,
            presence_penalty=0.1,
            frequency_penalty=0.2
        )
        ```
    """

    def __init__(
        self,
        stop_sequences: Optional[List[str]] = None,
        response_mime_type: Optional[str] = None,
        response_modalities: Optional[List[Modality]] = None,
        thinking_config: Optional[ThinkingConfig] = None,
        temperature: Optional[float] = None,
        top_p: Optional[float] = None,
        top_k: Optional[int] = None,
        candidate_count: Optional[int] = None,
        max_output_tokens: Optional[int] = None,
        response_logprobs: Optional[bool] = None,
        logprobs: Optional[int] = None,
        presence_penalty: Optional[float] = None,
        frequency_penalty: Optional[float] = None,
        seed: Optional[int] = None,
        audio_timestamp: Optional[bool] = None,
        media_resolution: Optional[MediaResolution] = None,
        speech_config: Optional[SpeechConfig] = None,
        enable_affective_dialog: Optional[bool] = None,
    ) -> None:
        """Initialize GenerationConfig with optional parameters.

        Args:
            stop_sequences (Optional[List[str]]):
                List of strings that will stop generation when encountered
            response_mime_type (Optional[str]):
                MIME type for the response format
            response_modalities (Optional[List[Modality]]):
                List of modalities to include in the response
            thinking_config (Optional[ThinkingConfig]):
                Configuration for reasoning/thinking capabilities
            temperature (Optional[float]):
                Controls randomness in generation (0.0-1.0)
            top_p (Optional[float]):
                Nucleus sampling parameter (0.0-1.0)
            top_k (Optional[int]):
                Top-k sampling parameter
            candidate_count (Optional[int]):
                Number of response candidates to generate
            max_output_tokens (Optional[int]):
                Maximum number of tokens to generate
            response_logprobs (Optional[bool]):
                Whether to return log probabilities
            logprobs (Optional[int]):
                Number of log probabilities to return per token
            presence_penalty (Optional[float]):
                Penalty for token presence (-2.0 to 2.0)
            frequency_penalty (Optional[float]):
                Penalty for token frequency (-2.0 to 2.0)
            seed (Optional[int]):
                Random seed for deterministic generation
            audio_timestamp (Optional[bool]):
                Whether to include timestamps in audio responses
            media_resolution (Optional[MediaResolution]):
                Resolution setting for media content
            speech_config (Optional[SpeechConfig]):
                Configuration for speech synthesis
            enable_affective_dialog (Optional[bool]):
                Whether to enable emotional dialog features
        """

    def __str__(self) -> str: ...

class HarmCategory:
    HarmCategoryUnspecified: "HarmCategory"
    HarmCategoryHateSpeech: "HarmCategory"
    HarmCategoryDangerousContent: "HarmCategory"
    HarmCategoryHarassment: "HarmCategory"
    HarmCategorySexuallyExplicit: "HarmCategory"
    HarmCategoryImageHate: "HarmCategory"
    HarmCategoryImageDangerousContent: "HarmCategory"
    HarmCategoryImageHarassment: "HarmCategory"
    HarmCategoryImageSexuallyExplicit: "HarmCategory"

class HarmBlockThreshold:
    HarmBlockThresholdUnspecified: "HarmBlockThreshold"
    BlockLowAndAbove: "HarmBlockThreshold"
    BlockMediumAndAbove: "HarmBlockThreshold"
    BlockOnlyHigh: "HarmBlockThreshold"
    BlockNone: "HarmBlockThreshold"
    Off: "HarmBlockThreshold"

class HarmBlockMethod:
    HarmBlockMethodUnspecified: "HarmBlockMethod"
    Severity: "HarmBlockMethod"
    Probability: "HarmBlockMethod"

class ModelArmorConfig:
    def __init__(
        self,
        prompt_template_name: Optional[str],
        response_template_name: Optional[str],
    ) -> None:
        """
        Args:
            prompt_template_name (Optional[str]):
                The name of the prompt template to use.
            response_template_name (Optional[str]):
                The name of the response template to use.
        """

    @property
    def prompt_template_name(self) -> Optional[str]: ...
    @property
    def response_template_name(self) -> Optional[str]: ...

class SafetySetting:
    category: HarmCategory
    threshold: HarmBlockThreshold
    method: Optional[HarmBlockMethod]

    def __init__(
        self,
        category: HarmCategory,
        threshold: HarmBlockThreshold,
        method: Optional[HarmBlockMethod] = None,
    ) -> None:
        """Initialize SafetySetting with required and optional parameters.

        Args:
            category (HarmCategory):
                The category of harm to protect against.
            threshold (HarmBlockThreshold):
                The threshold for blocking content.
            method (Optional[HarmBlockMethod]):
                The method used for blocking (if any).
        """

class Mode:
    ModeUnspecified: "Mode"
    Any: "Mode"
    Auto: "Mode"
    None_Mode: "Mode"  # type: ignore

class FunctionCallingConfig:
    @property
    def mode(self) -> Optional[Mode]: ...
    @property
    def allowed_function_names(self) -> Optional[list[str]]: ...
    def __init__(self, mode: Optional[Mode], allowed_function_names: Optional[list[str]]) -> None: ...

class LatLng:
    @property
    def latitude(self) -> float: ...
    @property
    def longitude(self) -> float: ...
    def __init__(self, latitude: float, longitude: float) -> None:
        """Initialize LatLng with latitude and longitude.

        Args:
            latitude (float):
                The latitude value.
            longitude (float):
                The longitude value.
        """

class RetrievalConfig:
    @property
    def lat_lng(self) -> LatLng: ...
    @property
    def language_code(self) -> str: ...
    def __init__(self, lat_lng: LatLng, language_code: str) -> None:
        """Initialize RetrievalConfig with latitude/longitude and language code.

        Args:
            lat_lng (LatLng):
                The latitude and longitude configuration.
            language_code (str):
                The language code for the retrieval.
        """

class ToolConfig:
    @property
    def function_calling_config(self) -> Optional[FunctionCallingConfig]: ...
    @property
    def retrieval_config(self) -> Optional[RetrievalConfig]: ...
    def __init__(
        self,
        function_calling_config: Optional[FunctionCallingConfig],
        retrieval_config: Optional[RetrievalConfig],
    ) -> None: ...

class GeminiSettings:
    def __init__(
        self,
        labels: Optional[dict[str, str]] = None,
        tool_config: Optional[ToolConfig] = None,
        generation_config: Optional[GenerationConfig] = None,
        safety_settings: Optional[list[SafetySetting]] = None,
        model_armor_config: Optional[ModelArmorConfig] = None,
        extra_body: Optional[dict] = None,
    ) -> None:
        """Settings to pass to the Gemini API when creating a request

        Reference:
            https://cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1beta1/projects.locations.endpoints/generateContent

        Args:
            labels (Optional[dict[str, str]]):
                An optional dictionary of labels for the settings.
            tool_config (Optional[ToolConfig]):
                Configuration for tools like function calling and retrieval.
            generation_config (Optional[GenerationConfig]):
                Configuration for content generation parameters.
            safety_settings (Optional[list[SafetySetting]]):
                List of safety settings to apply.
            model_armor_config (Optional[ModelArmorConfig]):
                Configuration for model armor templates.
            extra_body (Optional[dict]):
                Additional configuration as a dictionary.
        """

    @property
    def labels(self) -> Optional[dict[str, str]]: ...
    @property
    def tool_config(self) -> Optional[ToolConfig]: ...
    @property
    def generation_config(self) -> Optional[GenerationConfig]: ...
    @property
    def safety_settings(self) -> Optional[list[SafetySetting]]: ...
    @property
    def model_armor_config(self) -> Optional[ModelArmorConfig]: ...
    @property
    def extra_body(self) -> Optional[dict]: ...
    def __str__(self) -> str: ...

class EmbeddingTaskType:
    TaskTypeUnspecified = "EmbeddingTaskType"
    RetrievalQuery = "EmbeddingTaskType"
    RetrievalDocument = "EmbeddingTaskType"
    SemanticSimilarity = "EmbeddingTaskType"
    Classification = "EmbeddingTaskType"
    Clustering = "EmbeddingTaskType"
    QuestionAnswering = "EmbeddingTaskType"
    FactVerification = "EmbeddingTaskType"
    CodeRetrievalQuery = "EmbeddingTaskType"

class GeminiEmbeddingConfig:
    def __init__(
        self,
        model: Optional[str] = None,
        output_dimensionality: Optional[int] = None,
        task_type: Optional[EmbeddingTaskType | str] = None,
    ) -> None:
        """Configuration to pass to the Gemini Embedding API when creating a request


        Args:
            model (Optional[str]):
                The embedding model to use. If not specified, the default gemini model will be used.
            output_dimensionality (Optional[int]):
                The output dimensionality of the embeddings. If not specified, a default value will be used.
            task_type (Optional[EmbeddingTaskType]):
                The type of embedding task to perform. If not specified, the default gemini task type will be used.
        """

class ContentEmbedding:
    @property
    def values(self) -> List[float]: ...

class GeminiEmbeddingResponse:
    @property
    def embedding(self) -> ContentEmbedding: ...

class PredictResponse:
    @property
    def predictions(self) -> List[dict]: ...
    @property
    def metadata(self) -> Any: ...
    @property
    def deployed_model_id(self) -> str: ...
    @property
    def model(self) -> str: ...
    @property
    def model_version_id(self) -> str: ...
    @property
    def model_display_name(self) -> str: ...
    def __str__(self): ...

class PredictRequest:
    def __init__(self, instances: List[dict], parameters: Optional[dict] = None) -> None:
        """Request to pass to the Vertex Predict API when creating a request

        Args:
            instances (List[dict]):
                A list of instances to be sent in the request.
            parameters (Optional[dict]):
                Optional parameters for the request.
        """

    @property
    def instances(self) -> List[dict]: ...
    @property
    def parameters(self) -> dict: ...
    def __str__(self): ...

class AudioParam:
    def __init__(self, format: str, voice: str) -> None: ...
    @property
    def format(self) -> str: ...
    @property
    def voice(self) -> str: ...

class ContentPart:
    def __init__(self, type: str, text: str) -> None: ...
    @property
    def type(self) -> str: ...
    @property
    def text(self) -> str: ...

class Content:
    def __init__(
        self,
        text: Optional[str] = None,
        parts: Optional[List[ContentPart]] = None,
    ) -> None: ...

class Prediction:
    def __init__(self, type: str, content: Content) -> None: ...
    @property
    def type(self) -> str: ...
    @property
    def content(self) -> Content: ...

class StreamOptions:
    def __init__(
        self,
        include_obfuscation: Optional[bool] = None,
        include_usage: Optional[bool] = None,
    ) -> None: ...
    @property
    def include_obfuscation(self) -> Optional[bool]: ...
    @property
    def include_usage(self) -> Optional[bool]: ...

class ToolChoiceMode:
    NA: "ToolChoiceMode"
    Auto: "ToolChoiceMode"
    Required: "ToolChoiceMode"

class FunctionChoice:
    def __init__(self, name: str) -> None: ...
    @property
    def name(self) -> str: ...

class FunctionToolChoice:
    def __init__(self, function: FunctionChoice) -> None: ...
    @property
    def function(self) -> FunctionChoice: ...
    @property
    def type(self) -> str: ...

class CustomChoice:
    def __init__(self, name: str) -> None: ...
    @property
    def name(self) -> str: ...

class CustomToolChoice:
    def __init__(self, custom: CustomChoice) -> None: ...
    @property
    def custom(self) -> CustomChoice: ...
    @property
    def type(self) -> str: ...

class ToolDefinition:
    def __init__(self, function_name: str) -> None: ...
    @property
    def function_name(self) -> str: ...
    @property
    def type(self) -> str: ...

class AllowedToolsMode:
    Auto: "AllowedToolsMode"
    Required: "AllowedToolsMode"

class InnerAllowedTools:
    @property
    def mode(self) -> AllowedToolsMode: ...
    @property
    def tools(self) -> List[ToolDefinition]: ...

class AllowedTools:
    def __init__(self, mode: AllowedToolsMode, tools: List[ToolDefinition]) -> None: ...
    @property
    def type(self) -> str: ...
    @property
    def allowed_tools(self) -> InnerAllowedTools: ...

class ToolChoice:
    Mode: "ToolChoice"
    Function: "ToolChoice"
    Custom: "ToolChoice"
    Allowed: "ToolChoice"

    @staticmethod
    def from_mode(mode: AllowedToolsMode) -> "ToolChoice": ...
    @staticmethod
    def from_function(function_name: str) -> "ToolChoice": ...
    @staticmethod
    def from_custom(custom_name: str) -> "ToolChoice": ...
    @staticmethod
    def from_allowed_tools(allowed_tools: AllowedTools) -> "ToolChoice": ...

class FunctionDefinition:
    def __init__(
        self,
        name: str,
        description: Optional[str] = None,
        parameters: Optional[dict] = None,
        strict: Optional[bool] = None,
    ) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def description(self) -> Optional[str]: ...
    @property
    def parameters(self) -> Optional[dict]: ...
    @property
    def strict(self) -> Optional[bool]: ...

class FunctionTool:
    def __init__(self, function: FunctionDefinition, type: str) -> None: ...
    @property
    def function(self) -> FunctionDefinition: ...
    @property
    def type(self) -> str: ...

class TextFormat:
    def __init__(self, type: str) -> None: ...
    @property
    def type(self) -> str: ...

class Grammar:
    def __init__(self, definition: str, syntax: str) -> None: ...
    @property
    def definition(self) -> str: ...
    @property
    def syntax(self) -> str: ...

class GrammarFormat:
    def __init__(self, grammar: Grammar, type: str) -> None: ...
    @property
    def type(self) -> str: ...
    @property
    def grammar(self) -> Grammar: ...

class CustomToolFormat:
    def __init__(
        self,
        type: Optional[str] = None,
        grammar: Optional[Grammar] = None,
    ) -> None: ...

class CustomDefinition:
    def __init__(
        self,
        name: str,
        description: Optional[str] = None,
        format: Optional[CustomToolFormat] = None,
    ) -> None: ...
    @property
    def name(self) -> str: ...
    @property
    def description(self) -> Optional[str]: ...
    @property
    def format(self) -> Optional[CustomToolFormat]: ...

class CustomTool:
    def __init__(self, custom: CustomDefinition, type: str) -> None: ...

class Tool:
    def __init__(
        self,
        function: Optional[FunctionTool] = None,
        custom: Optional[CustomTool] = None,
    ) -> None: ...

class OpenAIChatSettings:
    """OpenAI chat completion settings configuration.

    This class provides configuration options for OpenAI chat completions,
    including model parameters, tool usage, and request options.

    Examples:
        >>> settings = OpenAIChatSettings(
        ...     temperature=0.7,
        ...     max_completion_tokens=1000,
        ...     stream=True
        ... )
        >>> settings.temperature = 0.5
    """

    def __init__(
        self,
        *,
        max_completion_tokens: Optional[int] = None,
        temperature: Optional[float] = None,
        top_p: Optional[float] = None,
        top_k: Optional[int] = None,
        frequency_penalty: Optional[float] = None,
        timeout: Optional[float] = None,
        parallel_tool_calls: Optional[bool] = None,
        seed: Optional[int] = None,
        logit_bias: Optional[Dict[str, int]] = None,
        stop_sequences: Optional[List[str]] = None,
        logprobs: Optional[bool] = None,
        audio: Optional[AudioParam] = None,
        metadata: Optional[Dict[str, str]] = None,
        modalities: Optional[List[str]] = None,
        n: Optional[int] = None,
        prediction: Optional[Prediction] = None,
        presence_penalty: Optional[float] = None,
        prompt_cache_key: Optional[str] = None,
        reasoning_effort: Optional[str] = None,
        safety_identifier: Optional[str] = None,
        service_tier: Optional[str] = None,
        store: Optional[bool] = None,
        stream: Optional[bool] = None,
        stream_options: Optional[StreamOptions] = None,
        tool_choice: Optional[ToolChoice] = None,
        tools: Optional[List[Tool]] = None,
        top_logprobs: Optional[int] = None,
        verbosity: Optional[str] = None,
        extra_body: Optional[Any] = None,
    ) -> None:
        """Initialize OpenAI chat settings.

        Args:
            max_completion_tokens (Optional[int]):
                Maximum number of tokens to generate
            temperature (Optional[float]):
                Sampling temperature (0.0 to 2.0)
            top_p (Optional[float]):
                Nucleus sampling parameter
            top_k (Optional[int]):
                Top-k sampling parameter
            frequency_penalty (Optional[float]):
                Frequency penalty (-2.0 to 2.0)
            timeout (Optional[float]):
                Request timeout in seconds
            parallel_tool_calls (Optional[bool]):
                Whether to enable parallel tool calls
            seed (Optional[int]):
                Random seed for deterministic outputs
            logit_bias (Optional[Dict[str, int]]):
                Token bias modifications
            stop_sequences (Optional[List[str]]):
                Sequences where generation should stop
            logprobs (Optional[bool]):
                Whether to return log probabilities
            audio (Optional[AudioParam]):
                Audio generation parameters
            metadata (Optional[Dict[str, str]]):
                Additional metadata for the request
            modalities (Optional[List[str]]):
                List of modalities to use
            n (Optional[int]):
                Number of completions to generate
            prediction (Optional[Prediction]):
                Prediction configuration
            presence_penalty (Optional[float]):
                Presence penalty (-2.0 to 2.0)
            prompt_cache_key (Optional[str]):
                Key for prompt caching
            reasoning_effort (Optional[str]):
                Reasoning effort level
            safety_identifier (Optional[str]):
                Safety configuration identifier
            service_tier (Optional[str]):
                Service tier to use
            store (Optional[bool]):
                Whether to store the conversation
            stream (Optional[bool]):
                Whether to stream the response
            stream_options (Optional[StreamOptions]):
                Streaming configuration options
            tool_choice (Optional[ToolChoice]):
                Tool choice configuration
            tools (Optional[List[Tool]]):
                Available tools for the model
            top_logprobs (Optional[int]):
                Number of top log probabilities to return
            verbosity (Optional[str]):
                Verbosity level for the response
            extra_body (Optional[Any]):
                Additional request body parameters
        """

    def __str__(self) -> str:
        """Return string representation of the settings."""

class OpenAIEmbeddingConfig:
    """OpenAI embedding configuration settings."""

    def __init__(
        self,
        model: str,
        dimensions: Optional[int] = None,
        encoding_format: Optional[str] = None,
        user: Optional[str] = None,
    ) -> None:
        """Initialize OpenAI embedding configuration.

        Args:
            model (str):
                The embedding model to use.
            dimensions (Optional[int]):
                The output dimensionality of the embeddings.
            encoding_format (Optional[str]):
                The encoding format to use for the embeddings.
                Can be either "float" or "base64".
            user (Optional[str]):
                The user ID for the embedding request.
        """

    @property
    def model(self) -> str: ...
    @property
    def dimensions(self) -> Optional[int]: ...
    @property
    def encoding_format(self) -> Optional[str]: ...
    @property
    def user(self) -> Optional[str]: ...

class EmbeddingObject:
    @property
    def object(self) -> str: ...
    @property
    def embedding(self) -> List[float]: ...
    @property
    def index(self) -> int: ...

class UsageObject:
    @property
    def prompt_tokens(self) -> int: ...
    @property
    def total_tokens(self) -> int: ...

class OpenAIEmbeddingResponse:
    @property
    def object(self) -> str: ...
    @property
    def data(self) -> List[EmbeddingObject]: ...
    @property
    def usage(self) -> UsageObject: ...

class PromptTokenDetails:
    """Details about the prompt tokens used in a request."""

    @property
    def audio_tokens(self) -> int:
        """The number of audio tokens used in the request."""

    @property
    def cached_tokens(self) -> int:
        """The number of cached tokens used in the request."""

class CompletionTokenDetails:
    """Details about the completion tokens used in a model response."""

    @property
    def accepted_prediction_tokens(self) -> int:
        """The number of accepted prediction tokens used in the response."""

    @property
    def audio_tokens(self) -> int:
        """The number of audio tokens used in the response."""

    @property
    def reasoning_tokens(self) -> int:
        """The number of reasoning tokens used in the response."""

    @property
    def rejected_prediction_tokens(self) -> int:
        """The number of rejected prediction tokens used in the response."""

class Usage:
    """Usage statistics for a model response."""

    @property
    def completion_tokens(self) -> int:
        """The number of completion tokens used in the response."""

    @property
    def prompt_tokens(self) -> int:
        """The number of prompt tokens used in the request."""

    @property
    def total_tokens(self) -> int:
        """The total number of tokens used in the request and response."""

    @property
    def completion_tokens_details(self) -> CompletionTokenDetails:
        """Details about the completion tokens used in the response."""

    @property
    def prompt_tokens_details(self) -> "PromptTokenDetails":
        """Details about the prompt tokens used in the request."""

    @property
    def finish_reason(self) -> str:
        """The reason why the model stopped generating tokens"""

class ImageUrl:
    def __init__(
        self,
        url: str,
        kind: Literal["image-url"] = "image-url",
    ) -> None:
        """Create an ImageUrl object.

        Args:
            url (str):
                The URL of the image.
            kind (Literal["image-url"]):
                The kind of the content.
        """

    @property
    def url(self) -> str:
        """The URL of the image."""

    @property
    def kind(self) -> str:
        """The kind of the content."""

    @property
    def media_type(self) -> str:
        """The media type of the image URL."""

    @property
    def format(self) -> str:
        """The format of the image URL."""

class AudioUrl:
    def __init__(
        self,
        url: str,
        kind: Literal["audio-url"] = "audio-url",
    ) -> None:
        """Create an AudioUrl object.

        Args:
            url (str):
                The URL of the audio.
            kind (Literal["audio-url"]):
                The kind of the content.
        """

    @property
    def url(self) -> str:
        """The URL of the audio."""

    @property
    def kind(self) -> str:
        """The kind of the content."""

    @property
    def media_type(self) -> str:
        """The media type of the audio URL."""

    @property
    def format(self) -> str:
        """The format of the audio URL."""

class BinaryContent:
    def __init__(
        self,
        data: bytes,
        media_type: str,
        kind: str = "binary",
    ) -> None:
        """Create a BinaryContent object.

        Args:
            data (bytes):
                The binary data.
            media_type (str):
                The media type of the binary data.
            kind (str):
                The kind of the content
        """

    @property
    def media_type(self) -> str:
        """The media type of the binary content."""

    @property
    def format(self) -> str:
        """The format of the binary content."""

    @property
    def data(self) -> bytes:
        """The binary data."""

    @property
    def kind(self) -> str:
        """The kind of the content."""

class DocumentUrl:
    def __init__(
        self,
        url: str,
        kind: Literal["document-url"] = "document-url",
    ) -> None:
        """Create a DocumentUrl object.

        Args:
            url (str):
                The URL of the document.
            kind (Literal["document-url"]):
                The kind of the content.
        """

    @property
    def url(self) -> str:
        """The URL of the document."""

    @property
    def kind(self) -> str:
        """The kind of the content."""

    @property
    def media_type(self) -> str:
        """The media type of the document URL."""

    @property
    def format(self) -> str:
        """The format of the document URL."""

class Message:
    def __init__(self, content: str | ImageUrl | AudioUrl | BinaryContent | DocumentUrl) -> None:
        """Create a Message object.

        Args:
            content (str | ImageUrl | AudioUrl | BinaryContent | DocumentUrl):
                The content of the message.
        """

    @property
    def content(self) -> str | ImageUrl | AudioUrl | BinaryContent | DocumentUrl:
        """The content of the message"""

    def bind(self, name: str, value: str) -> "Message":
        """Bind context to a specific variable in the prompt. This is an immutable operation meaning that it
        will return a new Message object with the context bound.

            Example with Prompt that contains two messages

            ```python
                prompt = Prompt(
                    model="openai:gpt-4o",
                    message=[
                        "My prompt variable is ${variable}",
                        "This is another message",
                    ],
                    system_instruction="system_prompt",
                )
                bounded_prompt = prompt.message[0].bind("variable", "hello world").unwrap() # we bind "hello world" to "variable"
            ```

        Args:
            name (str):
                The name of the variable to bind.
            value (str):
                The value to bind the variable to.

        Returns:
            Message:
                The message with the context bound.
        """

    def bind_mut(self, name: str, value: str) -> "Message":
        """Bind context to a specific variable in the prompt. This is a mutable operation meaning that it
        will modify the current Message object.

            Example with Prompt that contains two messages

            ```python
                prompt = Prompt(
                    model="openai:gpt-4o",
                    message=[
                        "My prompt variable is ${variable}",
                        "This is another message",
                    ],
                    system_instruction="system_prompt",
                )
                prompt.message[0].bind_mut("variable", "hello world") # we bind "hello world" to "variable"
            ```

        Args:
            name (str):
                The name of the variable to bind.
            value (str):
                The value to bind the variable to.

        Returns:
            Message:
                The message with the context bound.
        """

    def unwrap(self) -> Any:
        """Unwrap the message content.

        Returns:
            A serializable representation of the message content, which can be a string, list, or dict.
        """

    def model_dump(self) -> Dict[str, Any]:
        """Unwrap the message content and serialize it to a dictionary.

        Returns:
            Dict[str, Any]:
                The message dictionary with keys "content" and "role".
        """

class ModelSettings:
    def __init__(self, settings: OpenAIChatSettings | GeminiSettings) -> None:
        """ModelSettings for configuring the model.

        Args:
            settings (OpenAIChatSettings | GeminiSettings):
                The settings to use for the model. Currently supports OpenAI and Gemini settings.
        """

    @property
    def settings(self) -> OpenAIChatSettings | GeminiSettings:
        """The settings to use for the model."""

    def model_dump_json(self) -> str:
        """The JSON representation of the model settings."""

class Prompt:
    def __init__(
        self,
        message: (
            str
            | Sequence[str | ImageUrl | AudioUrl | BinaryContent | DocumentUrl]
            | Message
            | List[Message]
            | List[Dict[str, Any]]
        ),
        model: str,
        provider: Provider | str,
        system_instruction: Optional[str | List[str]] = None,
        model_settings: Optional[ModelSettings | OpenAIChatSettings | GeminiSettings] = None,
        response_format: Optional[Any] = None,
    ) -> None:
        """Prompt for interacting with an LLM API.

        Args:
            message (str | Sequence[str | ImageUrl | AudioUrl | BinaryContent | DocumentUrl] | Message | List[Message]):
                The prompt to use.
            model (str):
                The model to use for the prompt
            provider (Provider | str):
                The provider to use for the prompt.
            system_instruction (Optional[str | List[str]]):
                The system prompt to use in the prompt.
            model_settings (None):
                The model settings to use for the prompt.
                Defaults to None which means no model settings will be used
            response_format (Optional[BaseModel | Score]):
                The response format to use for the prompt. This is used for Structured Outputs
                (https://platform.openai.com/docs/guides/structured-outputs?api-mode=chat).
                Currently, response_format only support Pydantic BaseModel classes and the PotatoHead Score class.
                The provided response_format will be parsed into a JSON schema.

        """

    @property
    def model(self) -> str:
        """The model to use for the prompt."""

    @property
    def provider(self) -> str:
        """The provider to use for the prompt."""

    @property
    def model_identifier(self) -> Any:
        """Concatenation of provider and model, used for identifying the model in the prompt. This
        is commonly used with pydantic_ai to identify the model to use for the agent.

        Example:
            ```python
                prompt = Prompt(
                    model="gpt-4o",
                    message="My prompt variable is ${variable}",
                    system_instruction="system_instruction",
                    provider="openai",
                )
                agent = Agent(
                    prompt.model_identifier, # "openai:gpt-4o"
                    system_instructions=prompt.system_instruction[0].unwrap(),
                )
            ```
        """

    @property
    def model_settings(self) -> ModelSettings:
        """The model settings to use for the prompt."""

    @property
    def message(
        self,
    ) -> List[Message]:
        """The user message to use in the prompt."""

    @property
    def system_instruction(self) -> List[Message]:
        """The system message to use in the prompt."""

    def save_prompt(self, path: Optional[Path] = None) -> None:
        """Save the prompt to a file.

        Args:
            path (Optional[Path]):
                The path to save the prompt to. If None, the prompt will be saved to
                the current working directory.
        """

    @staticmethod
    def from_path(path: Path) -> "Prompt":
        """Load a prompt from a file.

        Args:
            path (Path):
                The path to the prompt file.

        Returns:
            Prompt:
                The loaded prompt.
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "Prompt":
        """Validate the model JSON.

        Args:
            json_string (str):
                The JSON string to validate.
        Returns:
            Prompt:
                The prompt object.
        """

    def model_dump_json(self) -> str:
        """Dump the model to a JSON string.

        Returns:
            str:
                The JSON string.
        """

    def bind(
        self,
        name: Optional[str] = None,
        value: Optional[str | int | float | bool | list] = None,
        **kwargs: Any,
    ) -> "Prompt":
        """Bind context to a specific variable in the prompt. This is an immutable operation meaning that it
        will return a new Prompt object with the context bound. This will iterate over all user messages.

        Args:
            name (str):
                The name of the variable to bind.
            value (str | int | float | bool | list):
                The value to bind the variable to. Must be a JSON serializable type.
            **kwargs (Any):
                Additional keyword arguments to bind to the prompt. This can be used to bind multiple variables at once.

        Returns:
            Prompt:
                The prompt with the context bound.
        """

    def bind_mut(
        self,
        name: Optional[str] = None,
        value: Optional[str | int | float | bool | list] = None,
        **kwargs: Any,
    ) -> "Prompt":
        """Bind context to a specific variable in the prompt. This is a mutable operation meaning that it
        will modify the current Prompt object. This will iterate over all user messages.

        Args:
            name (str):
                The name of the variable to bind.
            value (str | int | float | bool | list):
                The value to bind the variable to. Must be a JSON serializable type.
            **kwargs (Any):
                Additional keyword arguments to bind to the prompt. This can be used to bind multiple variables at once.

        Returns:
            Prompt:
                The prompt with the context bound.
        """

    @property
    def response_json_schema(self) -> Optional[str]:
        """The JSON schema for the response if provided."""

    def __str__(self): ...

class Provider:
    OpenAI: "Provider"
    Gemini: "Provider"
    Vertex: "Provider"
    Google: "Provider"

class TaskStatus:
    Pending: "TaskStatus"
    Running: "TaskStatus"
    Completed: "TaskStatus"
    Failed: "TaskStatus"

class ResponseLogProbs:
    @property
    def token(self) -> str:
        """The token for which the log probabilities are calculated."""

    @property
    def logprob(self) -> float:
        """The log probability of the token."""

class LogProbs:
    @property
    def tokens(self) -> List[ResponseLogProbs]:
        """The log probabilities of the tokens in the response.
        This is primarily used for debugging and analysis purposes.
        """

    def __str__(self) -> str:
        """String representation of the log probabilities."""

class AgentResponse:
    @property
    def id(self) -> str:
        """The ID of the agent response."""

    @property
    def result(self) -> Any:
        """The result of the agent response. This can be a Pydantic BaseModel class or a supported potato_head response
        type such as `Score`. If neither is provided, the response json will be returned as a dictionary.
        """

    @property
    def token_usage(self) -> Usage:
        """Returns the token usage of the agent response if supported"""

    @property
    def log_probs(self) -> List["ResponseLogProbs"]:
        """Returns the log probabilities of the agent response if supported.
        This is primarily used for debugging and analysis purposes.
        """

class Task:
    def __init__(
        self,
        agent_id: str,
        prompt: Prompt,
        dependencies: List[str] = [],
        id: Optional[str] = None,
    ) -> None:
        """Create a Task object.

        Args:
            agent_id (str):
                The ID of the agent that will execute the task.
            prompt (Prompt):
                The prompt to use for the task.
            dependencies (List[str]):
                The dependencies of the task.
            id (Optional[str]):
                The ID of the task. If None, a random uuid7 will be generated.
        """

    @property
    def prompt(self) -> Prompt:
        """The prompt to use for the task."""

    @property
    def dependencies(self) -> List[str]:
        """The dependencies of the task."""

    @property
    def id(self) -> str:
        """The ID of the task."""

    @property
    def status(self) -> TaskStatus:
        """The status of the task."""

class TaskList:
    def __init__(self) -> None:
        """Create a TaskList object."""

class Agent:
    def __init__(
        self,
        provider: Provider | str,
        system_instruction: Optional[str | List[str] | Message | List[Message]] = None,
    ) -> None:
        """Create an Agent object.

        Args:
            provider (Provider | str):
                The provider to use for the agent. This can be a Provider enum or a string
                representing the provider.
            system_instruction (Optional[str | List[str] | Message | List[Message]]):
                The system message to use for the agent. This can be a string, a list of strings,
                a Message object, or a list of Message objects. If None, no system message will be used.
                This is added to all tasks that the agent executes. If a given task contains it's own
                system message, the agent's system message will be prepended to the task's system message.

        Example:
        ```python
            agent = Agent(
                provider=Provider.OpenAI,
                system_instruction="You are a helpful assistant.",
            )
        ```
        """

    @property
    def system_instruction(self) -> List[Message]:
        """The system message to use for the agent. This is a list of Message objects."""

    def execute_task(
        self,
        task: Task,
        output_type: Optional[Any] = None,
        model: Optional[str] = None,
    ) -> AgentResponse:
        """Execute a task.

        Args:
            task (Task):
                The task to execute.
            output_type (Optional[Any]):
                The output type to use for the task. This can either be a Pydantic `BaseModel` class
                or a supported PotatoHead response type such as `Score`.
            model (Optional[str]):
                The model to use for the task. If not provided, defaults to the `model` provided within
                the Task's prompt. If the Task's prompt does not have a model, an error will be raised.

        Returns:
            AgentResponse:
                The response from the agent after executing the task.
        """

    def execute_prompt(
        self,
        prompt: Prompt,
        output_type: Optional[Any] = None,
        model: Optional[str] = None,
    ) -> AgentResponse:
        """Execute a prompt.

        Args:
            prompt (Prompt):`
                The prompt to execute.
            output_type (Optional[Any]):
                The output type to use for the task. This can either be a Pydantic `BaseModel` class
                or a supported potato_head response type such as `Score`.
            model (Optional[str]):
                The model to use for the task. If not provided, defaults to the `model` provided within
                the Prompt. If the Prompt does not have a model, an error will be raised.

        Returns:
            AgentResponse:
                The response from the agent after executing the task.
        """

    @property
    def id(self) -> str:
        """The ID of the agent. This is a random uuid7 that is generated when the agent is created."""

ConfigT = TypeVar("ConfigT", OpenAIEmbeddingConfig, GeminiEmbeddingConfig, None)

class Embedder:
    """Class for creating embeddings."""

    def __init__(  # type: ignore
        self,
        provider: Provider | str,
        config: Optional[OpenAIEmbeddingConfig | GeminiEmbeddingConfig] = None,
    ) -> None:
        """Create an Embedder object.

        Args:
            provider (Provider | str):
                The provider to use for the embedder. This can be a Provider enum or a string
                representing the provider.
            config (Optional[OpenAIEmbeddingConfig | GeminiEmbeddingConfig]):
                The configuration to use for the embedder. This can be a Pydantic BaseModel class
                representing the configuration for the provider. If no config is provided,
                defaults to OpenAI provider configuration.
        """

    def embed(
        self,
        input: str | List[str] | PredictRequest,
    ) -> OpenAIEmbeddingResponse | GeminiEmbeddingResponse | PredictResponse:
        """Create embeddings for input.

        Args:
            input: The input to embed. Type depends on provider:
                - OpenAI/Gemini: str | List[str]
                - Vertex: PredictRequest

        Returns:
            Provider-specific response type.
            OpenAIEmbeddingResponse for OpenAI,
            GeminiEmbeddingResponse for Gemini,
            PredictResponse for Vertex.

        Examples:
            ```python
            ## OpenAI
            embedder = Embedder(Provider.OpenAI)
            response = embedder.embed(input="Test input")

            ## Gemini
            embedder = Embedder(Provider.Gemini, config=GeminiEmbeddingConfig(model="gemini-embedding-001"))
            response = embedder.embed(input="Test input")

            ## Vertex
            from potato_head.google import PredictRequest
            embedder = Embedder(Provider.Vertex)
            response = embedder.embed(input=PredictRequest(text="Test input"))
            ```
        """

class Workflow:
    def __init__(self, name: str) -> None:
        """Create a Workflow object.

        Args:
            name (str):
                The name of the workflow.
        """

    @property
    def name(self) -> str:
        """The name of the workflow."""

    @property
    def task_list(self) -> TaskList:
        """The tasks in the workflow."""

    @property
    def agents(self) -> Dict[str, Agent]:
        """The agents in the workflow."""

    @property
    def is_workflow(self) -> bool:
        """Returns True if the workflow is a valid workflow, otherwise False.
        This is used to determine if the workflow can be executed.
        """

    def __workflow__(self) -> str:
        """Returns a string representation of the workflow."""

    def add_task_output_types(self, task_output_types: Dict[str, Any]) -> None:
        """Add output types for tasks in the workflow. This is primarily used for
        when loading a workflow as python objects are not serializable.

        Args:
            task_output_types (Dict[str, Any]):
                A dictionary mapping task IDs to their output types.
                This can either be a Pydantic `BaseModel` class or a supported potato_head response type such as `Score`.
        """

    def add_task(self, task: Task, output_type: Optional[Any]) -> None:
        """Add a task to the workflow.

        Args:
            task (Task):
                The task to add to the workflow.
            output_type (Optional[Any]):
                The output type to use for the task. This can either be a Pydantic `BaseModel` class
                or a supported potato_head response type such as `Score`.
        """

    def add_tasks(self, tasks: List[Task]) -> None:
        """Add multiple tasks to the workflow.

        Args:
            tasks (List[Task]):
                The tasks to add to the workflow.
        """

    def add_agent(self, agent: Agent) -> None:
        """Add an agent to the workflow.

        Args:
            agent (Agent):
                The agent to add to the workflow.
        """

    def is_complete(self) -> bool:
        """Check if the workflow is complete.

        Returns:
            bool:
                True if the workflow is complete, False otherwise.
        """

    def pending_count(self) -> int:
        """Get the number of pending tasks in the workflow.

        Returns:
            int:
                The number of pending tasks in the workflow.
        """

    def execution_plan(self) -> Dict[str, List[str]]:
        """Get the execution plan for the workflow.

        Returns:
            Dict[str, List[str]]:
                A dictionary where the keys are task IDs and the values are lists of task IDs
                that the task depends on.
        """

    def run(
        self,
        global_context: Optional[Dict[str, Any]] = None,
    ) -> "WorkflowResult":
        """Run the workflow. This will execute all tasks in the workflow and return when all tasks are complete.

        Args:
            global_context (Optional[Dict[str, Any]]):
                A dictionary of global context to bind to the workflow.
                All tasks in the workflow will have this context bound to them.
        """

    def model_dump_json(self) -> str:
        """Dump the workflow to a JSON string.

        Returns:
            str:
                The JSON string.
        """

    @staticmethod
    def model_validate_json(json_string: str, output_types: Optional[Dict[str, Any]]) -> "Workflow":
        """Load a workflow from a JSON string.

        Args:
            json_string (str):
                The JSON string to validate.
            output_types (Optional[Dict[str, Any]]):
                A dictionary mapping task IDs to their output types.
                This can either be a Pydantic `BaseModel` class or a supported potato_head response type such as `Score`.

        Returns:
            Workflow:
                The workflow object.
        """

class PyTask:
    """Python-specific task interface for Task objects and results"""

    @property
    def prompt(self) -> Prompt:
        """The prompt to use for the task."""

    @property
    def dependencies(self) -> List[str]:
        """The dependencies of the task."""

    @property
    def id(self) -> str:
        """The ID of the task."""

    @property
    def agent_id(self) -> str:
        """The ID of the agent that will execute the task."""

    @property
    def status(self) -> TaskStatus:
        """The status of the task."""

    @property
    def result(self) -> Optional[AgentResponse]:
        """The result of the task if it has been executed, otherwise None."""

    def __str__(self) -> str: ...

class ChatResponse:
    def to_py(self) -> Any:
        """Convert the ChatResponse to it's Python representation."""

    def __str__(self) -> str:
        """Return a string representation of the ChatResponse."""

class EventDetails:
    @property
    def prompt(self) -> Optional[Prompt]:
        """The prompt used for the task."""

    @property
    def response(self) -> Optional[ChatResponse]:
        """The response from the agent after executing the task."""

    @property
    def duration(self) -> Optional[timedelta]:
        """The duration of the task execution."""

    @property
    def start_time(self) -> Optional[datetime]:
        """The start time of the task execution."""

    @property
    def end_time(self) -> Optional[datetime]:
        """The end time of the task execution."""

    @property
    def error(self) -> Optional[str]:
        """The error message if the task failed, otherwise None."""

class TaskEvent:
    @property
    def id(self) -> str:
        """The ID of the event"""

    @property
    def workflow_id(self) -> str:
        """The ID of the workflow that the task is part of."""

    @property
    def task_id(self) -> str:
        """The ID of the task that the event is associated with."""

    @property
    def status(self) -> TaskStatus:
        """The status of the task at the time of the event."""

    @property
    def timestamp(self) -> datetime:
        """The timestamp of the event. This is the time when the event occurred."""

    @property
    def updated_at(self) -> datetime:
        """The timestamp of when the event was last updated. This is useful for tracking changes to the event."""

    @property
    def details(self) -> EventDetails:
        """Additional details about the event. This can include information such as error messages or other relevant data."""

class WorkflowResult:
    @property
    def tasks(self) -> Dict[str, PyTask]:
        """The tasks in the workflow result."""

    @property
    def events(self) -> List[TaskEvent]:
        """The events that occurred during the workflow execution. This is a list of dictionaries
        where each dictionary contains information about the event such as the task ID, status, and timestamp.
        """

class Score:
    """A class representing a score with a score value and a reason. This is typically used
    as a response type for tasks/prompts that require scoring or evaluation of results.

    Example:
    ```python
        Prompt(
            model="openai:gpt-4o",
            message="What is the score of this response?",
            system_instruction="system_prompt",
            response_format=Score,
        )
    ```
    """

    @property
    def score(self) -> int:
        """The score value."""

    @property
    def reason(self) -> str:
        """The reason for the score."""

    @staticmethod
    def model_validate_json(json_string: str) -> "Score":
        """Validate the score JSON.

        Args:
            json_string (str):
                The JSON string to validate.

        Returns:
            Score:
                The score object.
        """

    def __str__(self): ...

#################
# _scouter.types
#################

class DriftType:
    Spc: "DriftType"
    Psi: "DriftType"
    Custom: "DriftType"
    LLM = "DriftType"

    def value(self) -> str: ...
    @staticmethod
    def from_value(value: str) -> "DriftType": ...

class CommonCrons:
    Every1Minute: "CommonCrons"
    Every5Minutes: "CommonCrons"
    Every15Minutes: "CommonCrons"
    Every30Minutes: "CommonCrons"
    EveryHour: "CommonCrons"
    Every6Hours: "CommonCrons"
    Every12Hours: "CommonCrons"
    EveryDay: "CommonCrons"
    EveryWeek: "CommonCrons"

    @property
    def cron(self) -> str:
        """Return the cron"""

    def get_next(self) -> str:
        """Return the next cron time"""

class ScouterDataType:
    Pandas: "ScouterDataType"
    Polars: "ScouterDataType"
    Numpy: "ScouterDataType"
    Arrow: "ScouterDataType"
    LLM: "ScouterDataType"

class CompressionType:
    NA: "CompressionType"
    Gzip: "CompressionType"
    Snappy: "CompressionType"
    Lz4: "CompressionType"
    Zstd: "CompressionType"

class ConsoleDispatchConfig:
    def __init__(self):
        """Initialize alert config"""

    @property
    def enabled(self) -> bool:
        """Return the alert dispatch type"""

class SlackDispatchConfig:
    def __init__(self, channel: str):
        """Initialize alert config

        Args:
            channel:
                Slack channel name for where alerts will be reported
        """

    @property
    def channel(self) -> str:
        """Return the slack channel name"""

    @channel.setter
    def channel(self, channel: str) -> None:
        """Set the slack channel name for where alerts will be reported"""

class OpsGenieDispatchConfig:
    def __init__(self, team: str):
        """Initialize alert config

        Args:
            team:
                Opsegenie team to be notified in the event of drift
        """

    @property
    def team(self) -> str:
        """Return the opesgenie team name"""

    @team.setter
    def team(self, team: str) -> None:
        """Set the opesgenie team name"""

class AlertDispatchType:
    Slack: "AlertDispatchType"
    OpsGenie: "AlertDispatchType"
    Console: "AlertDispatchType"

    @staticmethod
    def to_string() -> str:
        """Return the string representation of the alert dispatch type"""

DispatchConfigType = ConsoleDispatchConfig | SlackDispatchConfig | OpsGenieDispatchConfig

class AlertZone:
    Zone1: "AlertZone"
    Zone2: "AlertZone"
    Zone3: "AlertZone"
    Zone4: "AlertZone"
    NotApplicable: "AlertZone"

class SpcAlertType:
    OutOfBounds = "SpcAlertType"
    Consecutive = "SpcAlertType"
    Alternating = "SpcAlertType"
    AllGood = "SpcAlertType"
    Trend = "SpcAlertType"

class SpcAlertRule:
    def __init__(
        self,
        rule: str = "8 16 4 8 2 4 1 1",
        zones_to_monitor: List[AlertZone] = [
            AlertZone.Zone1,
            AlertZone.Zone2,
            AlertZone.Zone3,
            AlertZone.Zone4,
        ],
    ) -> None:
        """Initialize alert rule

        Args:
            rule:
                Rule to use for alerting. Eight digit integer string.
                Defaults to '8 16 4 8 2 4 1 1'
            zones_to_monitor:
                List of zones to monitor. Defaults to all zones.
        """

    @property
    def rule(self) -> str:
        """Return the alert rule"""

    @rule.setter
    def rule(self, rule: str) -> None:
        """Set the alert rule"""

    @property
    def zones_to_monitor(self) -> List[AlertZone]:
        """Return the zones to monitor"""

    @zones_to_monitor.setter
    def zones_to_monitor(self, zones_to_monitor: List[AlertZone]) -> None:
        """Set the zones to monitor"""

class PsiNormalThreshold:
    def __init__(self, alpha: float = 0.05):
        """Initialize PSI threshold using normal approximation.

        Uses the asymptotic normal distribution of PSI to calculate critical values
        for population drift detection.

        Args:
            alpha: Significance level (0.0 to 1.0, exclusive). Common values:
                   0.05 (95% confidence), 0.01 (99% confidence)

        Raises:
            ValueError: If alpha not in range (0.0, 1.0)
        """

    @property
    def alpha(self) -> float:
        """Statistical significance level for drift detection."""

    @alpha.setter
    def alpha(self, alpha: float) -> None:
        """Set significance level (must be between 0.0 and 1.0, exclusive)."""

class PsiChiSquareThreshold:
    def __init__(self, alpha: float = 0.05):
        """Initialize PSI threshold using chi-square approximation.

        Uses the asymptotic chi-square distribution of PSI.

        The chi-square method is generally more statistically rigorous than
        normal approximation, especially for smaller sample sizes.

        Args:
            alpha: Significance level (0.0 to 1.0, exclusive). Common values:
                   0.05 (95% confidence), 0.01 (99% confidence)

        Raises:
            ValueError: If alpha not in range (0.0, 1.0)
        """

    @property
    def alpha(self) -> float:
        """Statistical significance level for drift detection."""

    @alpha.setter
    def alpha(self, alpha: float) -> None:
        """Set significance level (must be between 0.0 and 1.0, exclusive)."""

class PsiFixedThreshold:
    def __init__(self, threshold: float = 0.25):
        """Initialize PSI threshold using a fixed value.

        Uses a predetermined PSI threshold value, similar to traditional
        "rule of thumb" approaches (e.g., 0.10 for moderate drift, 0.25
        for significant drift).

        Args:
            threshold: Fixed PSI threshold value (must be positive).
                      Common industry values: 0.10, 0.25

        Raises:
            ValueError: If threshold is not positive
        """

    @property
    def threshold(self) -> float:
        """Fixed PSI threshold value for drift detection."""

    @threshold.setter
    def threshold(self, threshold: float) -> None:
        """Set threshold value (must be positive)."""

PsiThresholdType = PsiNormalThreshold | PsiChiSquareThreshold | PsiFixedThreshold

class PsiAlertConfig:
    def __init__(
        self,
        dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None,
        schedule: Optional[str | CommonCrons] = None,
        features_to_monitor: List[str] = [],
        threshold: Optional[PsiThresholdType] = PsiChiSquareThreshold(),
    ):
        """Initialize alert config

        Args:
            dispatch_config:
                Alert dispatch configuration to use. Defaults to an internal "Console" type where
                the alerts will be logged to the console
            schedule:
                Schedule to run monitor. Defaults to daily at midnight
            features_to_monitor:
                List of features to monitor. Defaults to empty list, which means all features
            threshold:
                Configuration that helps determine how to calculate PSI critical values.
                Defaults to PsiChiSquareThreshold, which uses the chi-square distribution.
        """

    @property
    def dispatch_type(self) -> AlertDispatchType:
        """Return the alert dispatch type"""

    @property
    def dispatch_config(self) -> DispatchConfigType:
        """Return the dispatch config"""

    @property
    def threshold(self) -> PsiThresholdType:
        """Return the threshold config"""

    @property
    def schedule(self) -> str:
        """Return the schedule"""

    @schedule.setter
    def schedule(self, schedule: str) -> None:
        """Set the schedule"""

    @property
    def features_to_monitor(self) -> List[str]:
        """Return the features to monitor"""

    @features_to_monitor.setter
    def features_to_monitor(self, features_to_monitor: List[str]) -> None:
        """Set the features to monitor"""

class SpcAlertConfig:
    def __init__(
        self,
        rule: Optional[SpcAlertRule] = None,
        dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None,
        schedule: Optional[str | CommonCrons] = None,
        features_to_monitor: List[str] = [],
    ):
        """Initialize alert config

        Args:
            rule:
                Alert rule to use. Defaults to Standard
            dispatch_config:
                Alert dispatch config. Defaults to console
            schedule:
                Schedule to run monitor. Defaults to daily at midnight
            features_to_monitor:
                List of features to monitor. Defaults to empty list, which means all features

        """

    @property
    def dispatch_type(self) -> AlertDispatchType:
        """Return the alert dispatch type"""

    @property
    def dispatch_config(self) -> DispatchConfigType:
        """Return the dispatch config"""

    @property
    def rule(self) -> SpcAlertRule:
        """Return the alert rule"""

    @rule.setter
    def rule(self, rule: SpcAlertRule) -> None:
        """Set the alert rule"""

    @property
    def schedule(self) -> str:
        """Return the schedule"""

    @schedule.setter
    def schedule(self, schedule: str) -> None:
        """Set the schedule"""

    @property
    def features_to_monitor(self) -> List[str]:
        """Return the features to monitor"""

    @features_to_monitor.setter
    def features_to_monitor(self, features_to_monitor: List[str]) -> None:
        """Set the features to monitor"""

class SpcAlert:
    def __init__(self, kind: SpcAlertType, zone: AlertZone):
        """Initialize alert"""

    @property
    def kind(self) -> SpcAlertType:
        """Alert kind"""

    @property
    def zone(self) -> AlertZone:
        """Zone associated with alert"""

    def __str__(self) -> str:
        """Return the string representation of the alert."""

class AlertThreshold:
    """
    Enum representing different alert conditions for monitoring metrics.

    Attributes:
        Below: Indicates that an alert should be triggered when the metric is below a threshold.
        Above: Indicates that an alert should be triggered when the metric is above a threshold.
        Outside: Indicates that an alert should be triggered when the metric is outside a specified range.
    """

    Below: "AlertThreshold"
    Above: "AlertThreshold"
    Outside: "AlertThreshold"

    @staticmethod
    def from_value(value: str) -> "AlertThreshold":
        """
        Creates an AlertThreshold enum member from a string value.

        Args:
            value (str): The string representation of the alert condition.

        Returns:
            AlertThreshold: The corresponding AlertThreshold enum member.
        """

class CustomMetricAlertCondition:
    def __init__(
        self,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Optional[float],
    ):
        """Initialize a CustomMetricAlertCondition instance.
        Args:
            alert_threshold (AlertThreshold): The condition that determines when an alert
                should be triggered. This could be comparisons like 'greater than',
                'less than', 'equal to', etc.
            alert_threshold_value (Optional[float], optional): A numerical boundary used in
                conjunction with the alert_threshold. This can be None for certain
                types of comparisons that don't require a fixed boundary.
        Example:
            alert_threshold = CustomMetricAlertCondition(AlertCondition.BELOW, 2.0)
        """

    @property
    def alert_threshold(self) -> AlertThreshold:
        """Return the alert_threshold"""

    @alert_threshold.setter
    def alert_threshold(self, alert_threshold: AlertThreshold) -> None:
        """Set the alert_threshold"""

    @property
    def alert_threshold_value(self) -> float:
        """Return the alert_threshold_value"""

    @alert_threshold_value.setter
    def alert_threshold_value(self, alert_threshold_value: float) -> None:
        """Set the alert_threshold_value"""

class CustomMetricAlertConfig:
    def __init__(
        self,
        dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None,
        schedule: Optional[str | CommonCrons] = None,
    ):
        """Initialize alert config

        Args:
            dispatch_config:
                Alert dispatch config. Defaults to console
            schedule:
                Schedule to run monitor. Defaults to daily at midnight

        """

    @property
    def dispatch_type(self) -> AlertDispatchType:
        """Return the alert dispatch type"""

    @property
    def dispatch_config(self) -> DispatchConfigType:
        """Return the dispatch config"""

    @property
    def schedule(self) -> str:
        """Return the schedule"""

    @schedule.setter
    def schedule(self, schedule: str) -> None:
        """Set the schedule"""

    @property
    def alert_conditions(self) -> dict[str, CustomMetricAlertCondition]:
        """Return the alert_condition that were set during metric definition"""

    @alert_conditions.setter
    def alert_conditions(self, alert_conditions: dict[str, CustomMetricAlertCondition]) -> None:
        """Update the alert_condition that were set during metric definition"""

class LLMAlertConfig:
    def __init__(
        self,
        dispatch_config: Optional[SlackDispatchConfig | OpsGenieDispatchConfig] = None,
        schedule: Optional[str | CommonCrons] = None,
    ):
        """Initialize alert config

        Args:
            dispatch_config:
                Alert dispatch config. Defaults to console
            schedule:
                Schedule to run monitor. Defaults to daily at midnight

        """

    @property
    def dispatch_type(self) -> AlertDispatchType:
        """Return the alert dispatch type"""

    @property
    def dispatch_config(self) -> DispatchConfigType:
        """Return the dispatch config"""

    @property
    def schedule(self) -> str:
        """Return the schedule"""

    @schedule.setter
    def schedule(self, schedule: str) -> None:
        """Set the schedule"""

    @property
    def alert_conditions(self) -> Optional[Dict[str, LLMMetricAlertCondition]]:
        """Return the alert conditions"""

class LLMMetricAlertCondition:
    def __init__(
        self,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Optional[float],
    ):
        """Initialize a LLMMetricAlertCondition instance.
        Args:
            alert_threshold (AlertThreshold):
                The condition that determines when an alert should be triggered.
                Must be one of the AlertThreshold enum members like Below, Above, or Outside.
            alert_threshold_value (Optional[float], optional):
                A numerical boundary used in conjunction with the alert_threshold.
                This can be None for certain types of comparisons that don't require a fixed boundary.
        Example:
            alert_threshold = LLMMetricAlertCondition(AlertCondition.BELOW, 2.0)
        """

    def __str__(self) -> str:
        """Return the string representation of LLMMetricAlertCondition."""

class LogLevel:
    Debug: "LogLevel"
    Info: "LogLevel"
    Warn: "LogLevel"
    Error: "LogLevel"
    Trace: "LogLevel"

class WriteLevel:
    Stdout: "WriteLevel"
    Stderror: "WriteLevel"

class LoggingConfig:
    show_threads: bool
    log_level: LogLevel
    write_level: WriteLevel
    use_json: bool

    def __init__(
        self,
        show_threads: bool = True,
        log_level: LogLevel = LogLevel.Info,
        write_level: WriteLevel = WriteLevel.Stdout,
        use_json: bool = False,
    ) -> None:
        """
        Logging configuration options.

        Args:
            show_threads:
                Whether to include thread information in log messages.
                Default is True.

            log_level:
                Log level for the logger.
                Default is LogLevel.Info.

            write_level:
                Write level for the logger.
                Default is WriteLevel.Stdout.

            use_json:
                Whether to write log messages in JSON format.
                Default is False.
        """

class RustyLogger:
    @staticmethod
    def setup_logging(config: Optional[LoggingConfig] = None) -> None:
        """Setup logging with the provided configuration.

        Args:
            config:
                Logging configuration options.
        """

    @staticmethod
    def get_logger(config: Optional[LoggingConfig] = None) -> "RustyLogger":
        """Get a logger with the provided name.

        Args:
            config:
                Logging configuration options.
        """

    def debug(self, message: str, *args: Any) -> None:
        """Log a debug message.

        Args:
            message:
                Message to log.

            args:
                Additional arguments to format the message.
        """

    def info(self, message: str, *args: Any) -> None:
        """Log an info message.

        Args:
            message:
                Message to log.

            args:
                Additional arguments to format the message.
        """

    def warn(self, message: str, *args: Any) -> None:
        """Log a warning message.

        Args:
            message:
                Message to log.

            args:
                Additional arguments to format the message.
        """

    def error(self, message: str, *args: Any) -> None:
        """Log an error message.

        Args:
            message:
                Message to log.

            args:
                Additional arguments to format the message.
        """

    def trace(self, message: str, *args: Any) -> None:
        """Log a trace message.

        Args:
            message:
                Message to log.

            args:
                Additional arguments to format the message.
        """

class TagRecord:
    """Represents a single tag record associated with an entity."""

    entity_type: str
    entity_id: str
    key: str
    value: str

class Attribute:
    """Represents a key-value attribute associated with a span."""

    key: str
    value: Any

class SpanEvent:
    """Represents an event within a span."""

    timestamp: datetime
    name: str
    attributes: List[Attribute]
    dropped_attributes_count: int

class SpanLink:
    """Represents a link to another span."""

    trace_id: str
    span_id: str
    trace_state: str
    attributes: List[Attribute]
    dropped_attributes_count: int

class TraceBaggageRecord:
    """Represents a single baggage record associated with a trace."""

    created_at: datetime
    trace_id: str
    scope: str
    key: str
    value: str

class TraceFilters:
    """A struct for filtering traces, generated from Rust pyclass."""

    service_name: Optional[str]
    has_errors: Optional[bool]
    status_code: Optional[int]
    start_time: Optional[datetime]
    end_time: Optional[datetime]
    limit: Optional[int]
    cursor_created_at: Optional[datetime]
    cursor_trace_id: Optional[str]

    def __init__(
        self,
        service_name: Optional[str] = None,
        has_errors: Optional[bool] = None,
        status_code: Optional[int] = None,
        start_time: Optional[datetime] = None,
        end_time: Optional[datetime] = None,
        limit: Optional[int] = None,
        cursor_created_at: Optional[datetime] = None,
        cursor_trace_id: Optional[str] = None,
    ) -> None:
        """Initialize trace filters.

        Args:
            service_name:
                Service name filter
            has_errors:
                Filter by presence of errors
            status_code:
                Filter by root span status code
            start_time:
                Start time boundary (UTC)
            end_time:
                End time boundary (UTC)
            limit:
                Maximum number of results to return
            cursor_created_at:
                Pagination cursor: created at timestamp
            cursor_trace_id:
                Pagination cursor: trace ID
        """

class TraceMetricBucket:
    """Represents aggregated trace metrics for a specific time bucket."""

    bucket_start: datetime
    trace_count: int
    avg_duration_ms: float
    p50_duration_ms: Optional[float]
    p95_duration_ms: Optional[float]
    p99_duration_ms: Optional[float]
    error_rate: float

class TraceListItem:
    """Represents a summary item for a trace in a list view."""

    trace_id: str
    service_name: str
    scope: str
    root_operation: Optional[str]
    start_time: datetime
    end_time: Optional[datetime]
    duration_ms: Optional[int]
    status_code: int
    status_message: Optional[str]
    span_count: Optional[int]
    has_errors: bool
    error_count: int
    created_at: datetime

class TraceSpan:
    """Detailed information for a single span within a trace."""

    trace_id: str
    span_id: str
    parent_span_id: Optional[str]
    span_name: str
    span_kind: Optional[str]
    start_time: datetime
    end_time: Optional[datetime]
    duration_ms: Optional[int]
    status_code: int
    status_message: Optional[str]
    attributes: List[Attribute]
    events: List[SpanEvent]
    links: List[SpanLink]
    depth: int
    path: List[str]
    root_span_id: str
    span_order: int
    input: Any
    output: Any

class TransportType:
    Kafka = "TransportType"
    RabbitMQ = "TransportType"
    Redis = "TransportType"
    HTTP = "TransportType"
    Grpc = "TransportType"

class HttpConfig:
    server_uri: str
    username: str
    password: str
    auth_token: str

    def __init__(
        self,
        server_uri: Optional[str] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        auth_token: Optional[str] = None,
    ) -> None:
        """HTTP configuration to use with the HTTPProducer.

        Args:
            server_uri:
                URL of the HTTP server to publish messages to.
                If not provided, the value of the HTTP_server_uri environment variable is used.

            username:
                Username for basic authentication.

            password:
                Password for basic authentication.

            auth_token:
                Authorization token to use for authentication.

        """

    def __str__(self): ...

class GrpcConfig:
    server_uri: str
    username: str
    password: str

    def __init__(
        self,
        server_uri: Optional[str] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
    ) -> None:
        """gRPC configuration to use with the GrpcProducer.

        Args:
            server_uri:
                URL of the gRPC server to publish messages to.
                If not provided, the value of the SCOUTER_GRPC_URI environment variable is used.

            username:
                Username for basic authentication.
                If not provided, the value of the SCOUTER_USERNAME environment variable is used.

            password:
                Password for basic authentication.
                If not provided, the value of the SCOUTER_PASSWORD environment variable is used.
        """

    def __str__(self): ...

class KafkaConfig:
    brokers: str
    topic: str
    compression_type: str
    message_timeout_ms: int
    message_max_bytes: int
    log_level: LogLevel
    config: Dict[str, str]
    max_retries: int
    transport_type: TransportType

    def __init__(
        self,
        username: Optional[str] = None,
        password: Optional[str] = None,
        brokers: Optional[str] = None,
        topic: Optional[str] = None,
        compression_type: Optional[str] = None,
        message_timeout_ms: int = 600_000,
        message_max_bytes: int = 2097164,
        log_level: LogLevel = LogLevel.Info,
        config: Dict[str, str] = {},
        max_retries: int = 3,
    ) -> None:
        """Kafka configuration for connecting to and publishing messages to Kafka brokers.

        This configuration supports both authenticated (SASL) and unauthenticated connections.
        When credentials are provided, SASL authentication is automatically enabled with
        secure defaults.

        Authentication Priority (first match wins):
            1. Direct parameters (username/password)
            2. Environment variables (KAFKA_USERNAME/KAFKA_PASSWORD)
            3. Configuration dictionary (sasl.username/sasl.password)

        SASL Security Defaults:
            - security.protocol: "SASL_SSL" (override via KAFKA_SECURITY_PROTOCOL env var)
            - sasl.mechanism: "PLAIN" (override via KAFKA_SASL_MECHANISM env var)

        Args:
            username:
                SASL username for authentication.
                Fallback: KAFKA_USERNAME environment variable.
            password:
                SASL password for authentication.
                Fallback: KAFKA_PASSWORD environment variable.
            brokers:
                Comma-separated list of Kafka broker addresses (host:port).
                Fallback: KAFKA_BROKERS environment variable.
                Default: "localhost:9092"
            topic:
                Target Kafka topic for message publishing.
                Fallback: KAFKA_TOPIC environment variable.
                Default: "scouter_monitoring"
            compression_type:
                Message compression algorithm.
                Options: "none", "gzip", "snappy", "lz4", "zstd"
                Default: "gzip"
            message_timeout_ms:
                Maximum time to wait for message delivery (milliseconds).
                Default: 600000 (10 minutes)
            message_max_bytes:
                Maximum message size in bytes.
                Default: 2097164 (~2MB)
            log_level:
                Logging verbosity for the Kafka producer.
                Default: LogLevel.Info
            config:
                Additional Kafka producer configuration parameters.
                See: https://kafka.apache.org/documentation/#producerconfigs
                Note: Direct parameters take precedence over config dictionary values.
            max_retries:
                Maximum number of retry attempts for failed message deliveries.
                Default: 3

        Examples:
            Basic usage (unauthenticated):
            ```python
            config = KafkaConfig(
                brokers="kafka1:9092,kafka2:9092",
                topic="my_topic"
            )
            ```

            SASL authentication:
            ```python
            config = KafkaConfig(
                username="my_user",
                password="my_password",
                brokers="secure-kafka:9093",
                topic="secure_topic"
            )
            ```

            Advanced configuration:
            ```python
            config = KafkaConfig(
                brokers="kafka:9092",
                compression_type="lz4",
                config={
                    "acks": "all",
                    "batch.size": "32768",
                    "linger.ms": "10"
                }
            )
            ```
        """

    def __str__(self): ...

class RabbitMQConfig:
    address: str
    queue: str
    max_retries: int
    transport_type: TransportType

    def __init__(
        self,
        host: Optional[str] = None,
        port: Optional[int] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        queue: Optional[str] = None,
        max_retries: int = 3,
    ) -> None:
        """RabbitMQ configuration to use with the RabbitMQProducer.

        Args:
            host:
                RabbitMQ host.
                If not provided, the value of the RABBITMQ_HOST environment variable is used.

            port:
                RabbitMQ port.
                If not provided, the value of the RABBITMQ_PORT environment variable is used.

            username:
                RabbitMQ username.
                If not provided, the value of the RABBITMQ_USERNAME environment variable is used.

            password:
                RabbitMQ password.
                If not provided, the value of the RABBITMQ_PASSWORD environment variable is used.

            queue:
                RabbitMQ queue to publish messages to.
                If not provided, the value of the RABBITMQ_QUEUE environment variable is used.

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.
        """

    def __str__(self): ...

class RedisConfig:
    address: str
    channel: str
    transport_type: TransportType

    def __init__(
        self,
        address: Optional[str] = None,
        chanel: Optional[str] = None,
    ) -> None:
        """Redis configuration to use with a Redis producer

        Args:
            address (str):
                Redis address.
                If not provided, the value of the REDIS_ADDR environment variable is used and defaults to
                "redis://localhost:6379".

            channel (str):
                Redis channel to publish messages to.

                If not provided, the value of the REDIS_CHANNEL environment variable is used and defaults to "scouter_monitoring".
        """

    def __str__(self): ...

class TracePaginationResponse:
    """Response structure for paginated trace list requests."""

    items: List[TraceListItem]

class TraceSpansResponse:
    """Response structure containing a list of spans for a trace."""

    spans: List[TraceSpan]

class TraceBaggageResponse:
    """Response structure containing trace baggage records."""

    baggage: List[TraceBaggageRecord]

class TraceMetricsRequest:
    """Request payload for fetching trace metrics."""

    space: Optional[str]
    name: Optional[str]
    version: Optional[str]
    start_time: datetime
    end_time: datetime
    bucket_interval: str

    def __init__(
        self,
        start_time: datetime,
        end_time: datetime,
        bucket_interval: str,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
    ) -> None:
        """Initialize trace metrics request.

        Args:
            start_time:
                Start time boundary (UTC)
            end_time:
                End time boundary (UTC)
            bucket_interval:
                The time interval for metric aggregation buckets (e.g., '1 minutes', '30 minutes')
            space:
                Model space filter
            name:
                Model name filter
            version:
                Model version filter
        """

class TraceMetricsResponse:
    """Response structure containing aggregated trace metrics."""

    metrics: List[TraceMetricBucket]

class TagsResponse:
    """Response structure containing a list of tag records."""

    tags: List[TagRecord]

class TimeInterval:
    FiveMinutes: "TimeInterval"
    FifteenMinutes: "TimeInterval"
    ThirtyMinutes: "TimeInterval"
    OneHour: "TimeInterval"
    ThreeHours: "TimeInterval"
    SixHours: "TimeInterval"
    TwelveHours: "TimeInterval"
    TwentyFourHours: "TimeInterval"
    TwoDays: "TimeInterval"
    FiveDays: "TimeInterval"

class DriftRequest:
    def __init__(
        self,
        uid: str,
        space: str,
        time_interval: TimeInterval,
        max_data_points: int,
        start_datetime: Optional[datetime] = None,
        end_datetime: Optional[datetime] = None,
    ) -> None:
        """Initialize drift request

        Args:
            uid:
                Unique identifier tied to drift profile
            space:
                Space associated with drift profile
            time_interval:
                Time window for drift request
            max_data_points:
                Maximum data points to return
            start_datetime:
                Optional start datetime for drift request
            end_datetime:
                Optional end datetime for drift request
        """

class ProfileStatusRequest:
    def __init__(self, name: str, space: str, version: str, drift_type: DriftType, active: bool) -> None:
        """Initialize profile status request

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            drift_type:
                Profile drift type. A (repo/name/version can be associated with more than one drift type)
            active:
                Whether to set the profile as active or inactive
        """

class GetProfileRequest:
    def __init__(self, name: str, space: str, version: str, drift_type: DriftType) -> None:
        """Initialize get profile request

        Args:
            name:
                Profile name
            space:
                Profile space
            version:
                Profile version
            drift_type:
                Profile drift type. A (repo/name/version can be associated with more than one drift type)
        """

class Alert:
    created_at: datetime
    entity_name: str
    alert: Dict[str, str]
    id: int
    active: bool

class DriftAlertPaginationRequest:
    def __init__(
        self,
        uid: str,
        active: bool = False,
        limit: Optional[int] = None,
        cursor_created_at: Optional[datetime] = None,
        cursor_id: Optional[int] = None,
        direction: Optional[Literal["next", "previous"]] = "previous",
        start_datetime: Optional[datetime] = None,
        end_datetime: Optional[datetime] = None,
    ) -> None:
        """Initialize drift alert request. Used for paginated alert retrieval.

        Args:
            uid:
                Unique identifier tied to drift profile
            active:
                Whether to get active alerts only
            limit:
                Limit for number of alerts to return
            cursor_created_at:
                Pagination cursor: created at timestamp
            cursor_id:
                Pagination cursor: alert ID
            direction:
                Pagination direction: "next" or "previous"
            start_datetime:
                Optional start datetime for alert filtering
            end_datetime:
                Optional end datetime for alert filtering
        """

class AlertCursor:
    created_at: datetime
    id: int

class DriftAlertPaginationResponse:
    items: List[Alert]
    has_next: bool
    next_cursor: Optional[AlertCursor]
    has_previous: bool
    previous_cursor: Optional[AlertCursor]

# Client
class ScouterClient:
    """Helper client for interacting with Scouter Server"""

    def __init__(self, config: Optional[HttpConfig] = None) -> None:
        """Initialize ScouterClient

        Args:
            config:
                HTTP configuration for interacting with the server.
        """

    def get_binned_drift(
        self,
        drift_request: DriftRequest,
        drift_type: DriftType,
    ) -> Any:
        """Get drift map from server

        Args:
            drift_request:
                DriftRequest object
            drift_type:
                Drift type for request

        Returns:
            Drift map of type BinnedMetrics | BinnedPsiFeatureMetrics | BinnedSpcFeatureMetrics
        """

    def register_profile(self, profile: Any, set_active: bool = False, deactivate_others: bool = False) -> bool:
        """Registers a drift profile with the server

        Args:
            profile:
                Drift profile
            set_active:
                Whether to set the profile as active or inactive
            deactivate_others:
                Whether to deactivate other profiles

        Returns:
            boolean
        """

    def update_profile_status(self, request: ProfileStatusRequest) -> bool:
        """Update profile status

        Args:
            request:
                ProfileStatusRequest

        Returns:
            boolean
        """

    def get_alerts(self, request: DriftAlertPaginationRequest) -> DriftAlertPaginationResponse:
        """Get alerts

        Args:
            request:
                DriftAlertPaginationRequest

        Returns:
            DriftAlertPaginationResponse
        """

    def download_profile(self, request: GetProfileRequest, path: Optional[Path]) -> str:
        """Download profile

        Args:
            request:
                GetProfileRequest
            path:
                Path to save profile

        Returns:
            Path to downloaded profile
        """

    def get_paginated_traces(self, filters: TraceFilters) -> TracePaginationResponse:
        """Get paginated traces
        Args:
            filters:
                TraceFilters object
        Returns:
            TracePaginationResponse
        """

    def refresh_trace_summary(self) -> bool:
        """Refresh trace summary cache

        Returns:
            boolean
        """

    def get_trace_spans(
        self,
        trace_id: str,
        service_name: Optional[str] = None,
    ) -> TraceSpansResponse:
        """Get trace spans

        Args:
            trace_id:
                Trace ID
            service_name:
                Service name

        Returns:
            TraceSpansResponse
        """

    def get_trace_baggage(self, trace_id: str) -> TraceBaggageResponse:
        """Get trace baggage

        Args:
            trace_id:
                Trace ID

        Returns:
            TraceBaggageResponse
        """

    def get_trace_metrics(self, request: TraceMetricsRequest) -> TraceMetricsResponse:
        """Get trace metrics

        Args:
            request:
                TraceMetricsRequest

        Returns:
            TraceMetricsResponse
        """

    def get_tags(self, entity_type: str, entity_id: str) -> TagsResponse:
        """Get tags for an entity

        Args:
            entity_type:
                Entity type
            entity_id:
                Entity ID

        Returns:
            TagsResponse
        """

class BinnedMetricStats:
    avg: float
    lower_bound: float
    upper_bound: float

    def __str__(self) -> str: ...

class BinnedMetric:
    metric: str
    created_at: List[datetime]
    stats: List[BinnedMetricStats]

    def __str__(self) -> str: ...

class BinnedMetrics:
    @property
    def metrics(self) -> Dict[str, BinnedMetric]: ...
    def __str__(self) -> str: ...
    def __getitem__(self, key: str) -> Optional[BinnedMetric]: ...

class BinnedPsiMetric:
    created_at: List[datetime]
    psi: List[float]
    overall_psi: float
    bins: Dict[int, float]

    def __str__(self) -> str: ...

class BinnedPsiFeatureMetrics:
    features: Dict[str, BinnedMetric]

    def __str__(self) -> str: ...

class SpcDriftFeature:
    created_at: List[datetime]
    values: List[float]

    def __str__(self) -> str: ...

class BinnedSpcFeatureMetrics:
    features: Dict[str, SpcDriftFeature]

    def __str__(self) -> str: ...

class ScouterTestServer:
    def __init__(
        self,
        cleanup: bool = True,
        rabbit_mq: bool = False,
        kafka: bool = False,
        openai: bool = False,
        base_path: Optional[Path] = None,
    ) -> None:
        """Instantiates the test server.

        When the test server is used as a context manager, it will start the server
        in a background thread and set the appropriate env vars so that the client
        can connect to the server. The server will be stopped when the context manager
        exits and the env vars will be reset.

        Args:
            cleanup (bool, optional):
                Whether to cleanup the server after the test. Defaults to True.
            rabbit_mq (bool, optional):
                Whether to use RabbitMQ as the transport. Defaults to False.
            kafka (bool, optional):
                Whether to use Kafka as the transport. Defaults to False.
            openai (bool, optional):
                Whether to create a mock OpenAITest server. Defaults to False.
            base_path (Optional[Path], optional):
                The base path for the server. Defaults to None. This is primarily
                used for testing loading attributes from a pyproject.toml file.
        """

    def start_server(self) -> None:
        """Starts the test server."""

    def stop_server(self) -> None:
        """Stops the test server."""

    def __enter__(self) -> "ScouterTestServer":
        """Starts the test server."""

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        """Stops the test server."""

    def set_env_vars_for_client(self) -> None:
        """Sets the env vars for the client to connect to the server."""

    def remove_env_vars_for_client(self) -> None:
        """Removes the env vars for the client to connect to the server."""

    @staticmethod
    def cleanup() -> None:
        """Cleans up the test server."""

class MockConfig:
    def __init__(self, **kwargs) -> None:
        """Mock configuration for the ScouterQueue

        Args:
            **kwargs: Arbitrary keyword arguments to set as attributes.
        """

class EntityType:
    Feature = "EntityType"
    Metric = "EntityType"

class RecordType:
    Spc = "RecordType"
    Psi = "RecordType"
    Observability = "RecordType"
    Custom = "RecordType"

class ServerRecord:
    Spc: "ServerRecord"
    Psi: "ServerRecord"
    Custom: "ServerRecord"
    Observability: "ServerRecord"

    def __init__(self, record: Any) -> None:
        """Initialize server record

        Args:
            record:
                Server record to initialize
        """

    @property
    def record(
        self,
    ) -> Union[SpcRecord, PsiRecord, CustomMetricRecord, ObservabilityMetrics]:
        """Return the drift server record."""

class ServerRecords:
    def __init__(self, records: List[ServerRecord]) -> None:
        """Initialize server records

        Args:
            records:
                List of server records
        """

    @property
    def records(self) -> List[ServerRecord]:
        """Return the drift server records."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

class SpcRecord:
    def __init__(
        self,
        uid: str,
        feature: str,
        value: float,
    ):
        """Initialize spc drift server record

        Args:
            uid:
                Unique identifier for the spc record.
                Must correspond to an existing entity in Scouter.
            feature:
                Feature name
            value:
                Feature value
        """

    @property
    def created_at(self) -> datetime:
        """Return the created at timestamp."""

    @property
    def uid(self) -> str:
        """Return the unique identifier."""

    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def value(self) -> float:
        """Return the sample value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class PsiRecord:
    def __init__(
        self,
        uid: str,
        feature: str,
        bin_id: int,
        bin_count: int,
    ):
        """Initialize spc drift server record

        Args:
            uid:
                Unique identifier for the psi record.
                Must correspond to an existing entity in Scouter.
            feature:
                Feature name
            bin_id:
                Bundle ID
            bin_count:
                Bundle ID
        """

    @property
    def created_at(self) -> datetime:
        """Return the created at timestamp."""

    @property
    def uid(self) -> str:
        """Returns the unique identifier."""

    @property
    def feature(self) -> str:
        """Return the feature."""

    @property
    def bin_id(self) -> int:
        """Return the bin id."""

    @property
    def bin_count(self) -> int:
        """Return the sample value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class CustomMetricRecord:
    def __init__(
        self,
        uid: str,
        metric: str,
        value: float,
    ):
        """Initialize spc drift server record

        Args:
            uid:
                Unique identifier for the metric record.
                Must correspond to an existing entity in Scouter.
            metric:
                Metric name
            value:
                Metric value
        """

    @property
    def created_at(self) -> datetime:
        """Return the created at timestamp."""

    @property
    def uid(self) -> str:
        """Returns the unique identifier."""

    @property
    def metric(self) -> str:
        """Return the metric name."""

    @property
    def value(self) -> float:
        """Return the metric value."""

    def __str__(self) -> str:
        """Return the string representation of the record."""

    def model_dump_json(self) -> str:
        """Return the json representation of the record."""

    def to_dict(self) -> Dict[str, str]:
        """Return the dictionary representation of the record."""

class QueueFeature:
    def __init__(self, name: str, value: Any) -> None:
        """Initialize feature. Will attempt to convert the value to it's corresponding feature type.
        Current support types are int, float, string.

        Args:
            name:
                Name of the feature
            value:
                Value of the feature. Can be an int, float, or string.

        Example:
            ```python
            feature = Feature("feature_1", 1) # int feature
            feature = Feature("feature_2", 2.0) # float feature
            feature = Feature("feature_3", "value") # string feature
            ```
        """

    @staticmethod
    def int(name: str, value: int) -> "QueueFeature":
        """Create an integer feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def float(name: str, value: float) -> "QueueFeature":
        """Create a float feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def string(name: str, value: str) -> "QueueFeature":
        """Create a string feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

    @staticmethod
    def categorical(name: str, value: str) -> "QueueFeature":
        """Create a categorical feature

        Args:
            name:
                Name of the feature
            value:
                Value of the feature
        """

class Features:
    def __init__(
        self,
        features: List[QueueFeature] | Dict[str, Union[int, float, str]],
    ) -> None:
        """Initialize a features class

        Args:
            features:
                List of features or a dictionary of key-value pairs.
                If a list, each item must be an instance of Feature.
                If a dictionary, each key is the feature name and each value is the feature value.
                Supported types for values are int, float, and string.

        Example:
            ```python
            # Passing a list of features
            features = Features(
                features=[
                    Feature.int("feature_1", 1),
                    Feature.float("feature_2", 2.0),
                    Feature.string("feature_3", "value"),
                ]
            )

            # Passing a dictionary (pydantic model) of features
            class MyFeatures(BaseModel):
                feature1: int
                feature2: float
                feature3: str

            my_features = MyFeatures(
                feature1=1,
                feature2=2.0,
                feature3="value",
            )

            features = Features(my_features.model_dump())
            ```
        """

    def __str__(self) -> str:
        """Return the string representation of the features"""

    @property
    def features(self) -> List[QueueFeature]:
        """Return the list of features"""

    @property
    def entity_type(self) -> EntityType:
        """Return the entity type"""

class Metric:
    def __init__(self, name: str, value: float | int) -> None:
        """Initialize metric

        Args:
            name:
                Name of the metric
            value:
                Value to assign to the metric. Can be an int or float but will be converted to float.
        """

    def __str__(self) -> str:
        """Return the string representation of the metric"""

class Metrics:
    def __init__(self, metrics: List[Metric] | Dict[str, Union[int, float]]) -> None:
        """Initialize metrics

        Args:
            metrics:
                List of metrics or a dictionary of key-value pairs.
                If a list, each item must be an instance of Metric.
                If a dictionary, each key is the metric name and each value is the metric value.


        Example:
            ```python

            # Passing a list of metrics
            metrics = Metrics(
                metrics=[
                    Metric("metric_1", 1.0),
                    Metric("metric_2", 2.5),
                    Metric("metric_3", 3),
                ]
            )

            # Passing a dictionary (pydantic model) of metrics
            class MyMetrics(BaseModel):
                metric1: float
                metric2: int

            my_metrics = MyMetrics(
                metric1=1.0,
                metric2=2,
            )

            metrics = Metrics(my_metrics.model_dump())
        """

    def __str__(self) -> str:
        """Return the string representation of the metrics"""

    @property
    def metrics(self) -> List["Metric"]:
        """Return the list of metrics"""

    @property
    def entity_type(self) -> EntityType:
        """Return the entity type"""

class Queue:
    """Individual queue associated with a drift profile"""

    def insert(self, entity: Union[Features, Metrics, LLMRecord]) -> None:
        """Insert a record into the queue

        Args:
            entity:
                Entity to insert into the queue.
                Can be an instance for Features, Metrics, or LLMRecord.

        Example:
            ```python
            features = Features(
                features=[
                    Feature("feature_1", 1),
                    Feature("feature_2", 2.0),
                    Feature("feature_3", "value"),
                ]
            )
            queue.insert(features)
            ```
        """

    @property
    def identifier(self) -> str:
        """Return the identifier of the queue"""

class ScouterQueue:
    """Main queue class for Scouter. Publishes drift records to the configured transport"""

    @staticmethod
    def from_path(
        path: Dict[str, Path],
        transport_config: Union[
            KafkaConfig,
            RabbitMQConfig,
            RedisConfig,
            HttpConfig,
            GrpcConfig,
        ],
    ) -> "ScouterQueue":
        """Initializes Scouter queue from one or more drift profile paths.

        ```
        ╔══════════════════════════════════════════════════════════════════════════╗
        ║                    SCOUTER QUEUE ARCHITECTURE                            ║
        ╠══════════════════════════════════════════════════════════════════════════╣
        ║                                                                          ║
        ║  Python Runtime (Client)                                                 ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  ScouterQueue.from_path()                                          │  ║
        ║  │    • Load drift profiles (SPC, PSI, Custom, LLM)                   │  ║
        ║  │    • Configure transport (Kafka, RabbitMQ, Redis, HTTP, gRPC)      │  ║
        ║  └───────────────────────────┬────────────────────────────────────────┘  ║
        ║                              │                                           ║
        ║                              ▼                                           ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  queue["profile_alias"].insert(Features | Metrics | LLMRecord)     │  ║
        ║  └───────────────────────────┬────────────────────────────────────────┘  ║
        ║                              │                                           ║
        ╚══════════════════════════════╪═══════════════════════════════════════════╝
                                       │
                                       │  Language Boundary
                                       │
        ╔══════════════════════════════╪═══════════════════════════════════════════╗
        ║  Rust Runtime (Producer)     ▼                                           ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  Queue<T> (per profile)                                            │  ║
        ║  │    • Buffer records in memory                                      │  ║
        ║  │    • Validate against drift profile schema                         │  ║
        ║  │    • Convert to ServerRecord format                                │  ║
        ║  └───────────────────────────┬────────────────────────────────────────┘  ║
        ║                              │                                           ║
        ║                              ▼                                           ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  Transport Producer                                                │  ║
        ║  │    • KafkaProducer    → Kafka brokers                              │  ║
        ║  │    • RabbitMQProducer → RabbitMQ exchange                          │  ║
        ║  │    • RedisProducer    → Redis pub/sub                              │  ║
        ║  │    • HttpProducer     → HTTP endpoint                              │  ║
        ║  │    • GrpcProducer     → gRPC server                                │  ║
        ║  └───────────────────────────┬────────────────────────────────────────┘  ║
        ║                              │                                           ║
        ╚══════════════════════════════╪═══════════════════════════════════════════╝
                                       │
                                       │  Network/Message Bus
                                       │
        ╔══════════════════════════════╪═══════════════════════════════════════════╗
        ║  Scouter Server              ▼                                           ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  Consumer (Kafka/RabbitMQ/Redis/HTTP/gRPC)                         │  ║
        ║  │    • Receive drift records                                         │  ║
        ║  │    • Deserialize & validate                                        │  ║
        ║  └───────────────────────────┬────────────────────────────────────────┘  ║
        ║                              │                                           ║
        ║                              ▼                                           ║
        ║  ┌────────────────────────────────────────────────────────────────────┐  ║
        ║  │  Processing Pipeline                                               │  ║
        ║  │    • Calculate drift metrics (SPC, PSI)                            │  ║
        ║  │    • Evaluate alert conditions                                     │  ║
        ║  │    • Store in PostgreSQL                                           │  ║
        ║  │    • Dispatch alerts (Slack, OpsGenie, Console)                    │  ║
        ║  └────────────────────────────────────────────────────────────────────┘  ║
        ║                                                                          ║
        ╚══════════════════════════════════════════════════════════════════════════╝
        ```
        Flow Summary:
            1. **Python Runtime**: Initialize queue with drift profiles and transport config
            2. **Insert Records**: Call queue["alias"].insert() with Features/Metrics/LLMRecord
            3. **Rust Queue**: Buffer and validate records against profile schema
            4. **Transport Producer**: Serialize and publish to configured transport
            5. **Network**: Records travel via Kafka/RabbitMQ/Redis/HTTP/gRPC
            6. **Scouter Server**: Consumer receives, processes, and stores records
            7. **Alerting**: Evaluate drift conditions and dispatch alerts if triggered

        Args:
            path (Dict[str, Path]):
                Dictionary of drift profile paths.
                Each key is a user-defined alias for accessing a queue.

                Supported profile types:
                    • SpcDriftProfile    - Statistical Process Control monitoring
                    • PsiDriftProfile    - Population Stability Index monitoring
                    • CustomDriftProfile - Custom metric monitoring
                    • LLMDriftProfile    - LLM evaluation monitoring

            transport_config (Union[KafkaConfig, RabbitMQConfig, RedisConfig, HttpConfig, GrpcConfig]):
                Transport configuration for the queue publisher.

                Available transports:
                    • KafkaConfig     - Apache Kafka message bus
                    • RabbitMQConfig  - RabbitMQ message broker
                    • RedisConfig     - Redis pub/sub
                    • HttpConfig      - Direct HTTP to Scouter server
                    • GrpcConfig      - Direct gRPC to Scouter server

        Returns:
            ScouterQueue:
                Configured queue with Rust-based producers for each drift profile.

        Examples:
            Basic SPC monitoring with Kafka:
                >>> queue = ScouterQueue.from_path(
                ...     path={"spc": Path("spc_drift_profile.json")},
                ...     transport_config=KafkaConfig(
                ...         brokers="localhost:9092",
                ...         topic="scouter_monitoring",
                ...     ),
                ... )
                >>> queue["spc"].insert(
                ...     Features(features=[
                ...         Feature("feature_1", 1.5),
                ...         Feature("feature_2", 2.3),
                ...     ])
                ... )

            Multi-profile monitoring with HTTP:
                >>> queue = ScouterQueue.from_path(
                ...     path={
                ...         "spc": Path("spc_profile.json"),
                ...         "psi": Path("psi_profile.json"),
                ...         "custom": Path("custom_profile.json"),
                ...     },
                ...     transport_config=HttpConfig(
                ...         server_uri="http://scouter-server:8000",
                ...     ),
                ... )
                >>> queue["psi"].insert(Features(...))
                >>> queue["custom"].insert(Metrics(...))

            LLM monitoring with gRPC:
                >>> queue = ScouterQueue.from_path(
                ...     path={"llm_eval": Path("llm_profile.json")},
                ...     transport_config=GrpcConfig(
                ...         server_uri="http://scouter-server:50051",
                ...         username="monitoring_user",
                ...         password="secure_password",
                ...     ),
                ... )
                >>> queue["llm_eval"].insert(
                ...     LLMRecord(context={"input": "...", "response": "..."})
                ... )
        """

    def __getitem__(self, key: str) -> Queue:
        """Get the queue for the specified key

        Args:
            key (str):
                Key to get the queue for

        """

    def shutdown(self) -> None:
        """Shutdown the queue. This will close and flush all queues and transports"""

    @property
    def transport_config(
        self,
    ) -> Union[KafkaConfig, RabbitMQConfig, RedisConfig, HttpConfig, MockConfig]:
        """Return the transport configuration used by the queue"""

class BaseModel(Protocol):
    """Protocol for pydantic BaseModel to ensure compatibility with context"""

    def model_dump(self) -> Dict[str, Any]:
        """Dump the model as a dictionary"""

    def model_dump_json(self) -> str:
        """Dump the model as a JSON string"""

    def __str__(self) -> str:
        """String representation of the model"""

class LLMRecord:
    """LLM record containing context tied to a Large Language Model interaction
    that is used to evaluate drift in LLM responses.


    Examples:
        >>> record = LLMRecord(
        ...     context={
        ...         "input": "What is the capital of France?",
        ...         "response": "Paris is the capital of France."
        ...     },
        ... )
        >>> print(record.context["input"])
        "What is the capital of France?"
    """

    prompt: Optional[Prompt]
    """Optional prompt configuration associated with this record."""

    entity_type: EntityType
    """Type of entity, always EntityType.LLM for LLMRecord instances."""

    def __init__(
        self,
        context: Context,
        prompt: Optional[Prompt | SerializedType] = None,
    ) -> None:
        """Creates a new LLM record to associate with an `LLMDriftProfile`.
        The record is sent to the `Scouter` server via the `ScouterQueue` and is
        then used to inject context into the evaluation prompts.

        Args:
            context:
                Additional context information as a dictionary or a pydantic BaseModel. During evaluation,
                this will be merged with the input and response data and passed to the assigned
                evaluation prompts. So if you're evaluation prompts expect additional context via
                bound variables (e.g., `${foo}`), you can pass that here as key value pairs.
                {"foo": "bar"}
            prompt:
                Optional prompt configuration associated with this record. Can be a Potatohead Prompt or
                a JSON-serializable type.

        Raises:
            TypeError: If context is not a dict or a pydantic BaseModel.

        """

    @property
    def context(self) -> Dict[str, Any]:
        """Get the contextual information.

        Returns:
            The context data as a Python object (deserialized from JSON).

        Raises:
            TypeError: If the stored JSON cannot be converted to a Python object.
        """

class LLMTestServer:
    """
    Mock server for OpenAI API.
    This class is used to simulate the OpenAI API for testing purposes.
    """

    def __init__(self): ...
    def __enter__(self):
        """
        Start the mock server.
        """

    def __exit__(self, exc_type, exc_value, traceback):
        """
        Stop the mock server.
        """

class LatencyMetrics:
    @property
    def p5(self) -> float:
        """5th percentile"""

    @property
    def p25(self) -> float:
        """25th percentile"""

    @property
    def p50(self) -> float:
        """50th percentile"""

    @property
    def p95(self) -> float:
        """95th percentile"""

    @property
    def p99(self) -> float:
        """99th percentile"""

class RouteMetrics:
    @property
    def route_name(self) -> str:
        """Return the route name"""

    @property
    def metrics(self) -> LatencyMetrics:
        """Return the metrics"""

    @property
    def request_count(self) -> int:
        """Request count"""

    @property
    def error_count(self) -> int:
        """Error count"""

    @property
    def error_latency(self) -> float:
        """Error latency"""

    @property
    def status_codes(self) -> Dict[int, int]:
        """Dictionary of status codes and counts"""

class ObservabilityMetrics:
    @property
    def space(self) -> str:
        """Return the space"""

    @property
    def name(self) -> str:
        """Return the name"""

    @property
    def version(self) -> str:
        """Return the version"""

    @property
    def request_count(self) -> int:
        """Request count"""

    @property
    def error_count(self) -> int:
        """Error count"""

    @property
    def route_metrics(self) -> List[RouteMetrics]:
        """Route metrics object"""

    def __str__(self) -> str:
        """Return the string representation of the observability metrics"""

    def model_dump_json(self) -> str:
        """Return the json representation of the observability metrics"""

class Observer:
    def __init__(self, uid: str) -> None:
        """Initializes an api metric observer

        Args:
            uid:
                Unique identifier for the observer
        """

    def increment(self, route: str, latency: float, status_code: int) -> None:
        """Increment the feature value

        Args:
            route:
                Route name
            latency:
                Latency of request
            status_code:
                Status code of request
        """

    def collect_metrics(self) -> Optional[ServerRecords]:
        """Collect metrics from observer"""

    def reset_metrics(self) -> None:
        """Reset the observer metrics"""

class FeatureMap:
    @property
    def features(self) -> Dict[str, Dict[str, int]]:
        """Return the feature map."""

    def __str__(self) -> str:
        """Return the string representation of the feature map."""

class SpcFeatureDriftProfile:
    @property
    def id(self) -> str:
        """Return the id."""

    @property
    def center(self) -> float:
        """Return the center."""

    @property
    def one_ucl(self) -> float:
        """Return the zone 1 ucl."""

    @property
    def one_lcl(self) -> float:
        """Return the zone 1 lcl."""

    @property
    def two_ucl(self) -> float:
        """Return the zone 2 ucl."""

    @property
    def two_lcl(self) -> float:
        """Return the zone 2 lcl."""

    @property
    def three_ucl(self) -> float:
        """Return the zone 3 ucl."""

    @property
    def three_lcl(self) -> float:
        """Return the zone 3 lcl."""

    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

class SpcDriftConfig:
    def __init__(
        self,
        space: str = "__missing__",
        name: str = "__missing__",
        version: str = "0.1.0",
        sample_size: int = 25,
        alert_config: SpcAlertConfig = SpcAlertConfig(),
        config_path: Optional[Path] = None,
    ):
        """Initialize monitor config

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version. Defaults to 0.1.0
            sample_size:
                Sample size
            alert_config:
                Alert configuration
            config_path:
                Optional path to load config from.
        """

    @property
    def sample_size(self) -> int:
        """Return the sample size."""

    @sample_size.setter
    def sample_size(self, sample_size: int) -> None:
        """Set the sample size."""

    @property
    def name(self) -> str:
        """Model Name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set model name"""

    @property
    def space(self) -> str:
        """Model space"""

    @space.setter
    def space(self, space: str) -> None:
        """Set model space"""

    @property
    def version(self) -> str:
        """Model version"""

    @version.setter
    def version(self, version: str) -> None:
        """Set model version"""

    @property
    def uid(self) -> str:
        """Unique identifier for the drift config"""

    @uid.setter
    def uid(self, uid: str) -> None:
        """Set unique identifier for the drift config"""

    @property
    def feature_map(self) -> Optional[FeatureMap]:
        """Feature map"""

    @property
    def alert_config(self) -> SpcAlertConfig:
        """Alert configuration"""

    @alert_config.setter
    def alert_config(self, alert_config: SpcAlertConfig) -> None:
        """Set alert configuration"""

    @property
    def drift_type(self) -> DriftType:
        """Drift type"""

    @staticmethod
    def load_from_json_file(path: Path) -> "SpcDriftConfig":
        """Load config from json file

        Args:
            path:
                Path to json file to load config from.
        """

    def __str__(self) -> str:
        """Return the string representation of the config."""

    def model_dump_json(self) -> str:
        """Return the json representation of the config."""

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        sample_size: Optional[int] = None,
        alert_config: Optional[SpcAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            sample_size:
                Sample size
            alert_config:
                Alert configuration
        """

class SpcDriftProfile:
    @property
    def uid(self) -> str:
        """Return the unique identifier for the drift profile"""

    @property
    def scouter_version(self) -> str:
        """Return scouter version used to create DriftProfile"""

    @property
    def features(self) -> Dict[str, SpcFeatureDriftProfile]:
        """Return the list of features."""

    @property
    def config(self) -> SpcDriftConfig:
        """Return the monitor config."""

    def model_dump_json(self) -> str:
        """Return json representation of drift profile"""

    def model_dump(self) -> Dict[str, Any]:
        """Return dictionary representation of drift profile"""

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift profile to json file

        Args:
            path:
                Optional path to save the drift profile. If None, outputs to `spc_drift_profile.json`


        Returns:
            Path to the saved json file
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "SpcDriftProfile":
        """Load drift profile from json

        Args:
            json_string:
                JSON string representation of the drift profile

        """

    @staticmethod
    def from_file(path: Path) -> "SpcDriftProfile":
        """Load drift profile from file

        Args:
            path: Path to the file
        """

    @staticmethod
    def model_validate(data: Dict[str, Any]) -> "SpcDriftProfile":
        """Load drift profile from dictionary

        Args:
            data:
                DriftProfile dictionary
        """

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        sample_size: Optional[int] = None,
        alert_config: Optional[SpcAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            sample_size:
                Sample size
            alert_config:
                Alert configuration
        """

    def __str__(self) -> str:
        """Sting representation of DriftProfile"""

class FeatureDrift:
    @property
    def samples(self) -> List[float]:
        """Return list of samples"""

    @property
    def drift(self) -> List[float]:
        """Return list of drift values"""

    def __str__(self) -> str:
        """Return string representation of feature drift"""

class SpcFeatureDrift:
    @property
    def samples(self) -> List[float]:
        """Return list of samples"""

    @property
    def drift(self) -> List[float]:
        """Return list of drift values"""

class SpcDriftMap:
    """Drift map of features"""

    @property
    def space(self) -> str:
        """Space to associate with drift map"""

    @property
    def name(self) -> str:
        """name to associate with drift map"""

    @property
    def version(self) -> str:
        """Version to associate with drift map"""

    @property
    def features(self) -> Dict[str, SpcFeatureDrift]:
        """Returns dictionary of features and their data profiles"""

    def __str__(self) -> str:
        """Return string representation of data drift"""

    def model_dump_json(self) -> str:
        """Return json representation of data drift"""

    @staticmethod
    def model_validate_json(json_string: str) -> "SpcDriftMap":
        """Load drift map from json file.

        Args:
            json_string:
                JSON string representation of the drift map
        """

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift map to json file

        Args:
            path:
                Optional path to save the drift map. If None, outputs to `spc_drift_map.json`

        Returns:
            Path to the saved json file

        """

    def to_numpy(self) -> Any:
        """Return drift map as a tuple of sample_array, drift_array and list of features"""

class Manual:
    def __init__(self, num_bins: int):
        """Manual equal-width binning strategy.

        Divides the feature range into a fixed number of equally sized bins.

        Args:
            num_bins:
                The exact number of bins to create.
        """

    @property
    def num_bins(self) -> int:
        """The number of bins you want created"""

    @num_bins.setter
    def num_bins(self, num_bins: int) -> None:
        """Set the number of bins you want created"""

class SquareRoot:
    def __init__(self):
        """Use the SquareRoot equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class Sturges:
    def __init__(self):
        """Use the Sturges equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class Rice:
    def __init__(self):
        """Use the Rice equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class Doane:
    def __init__(self):
        """Use the Doane equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class Scott:
    def __init__(self):
        """Use the Scott equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class TerrellScott:
    def __init__(self):
        """Use the Terrell-Scott equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

class FreedmanDiaconis:
    def __init__(self):
        """Use the Freedman–Diaconis equal-width method.

        For more information, please see: https://en.wikipedia.org/wiki/Histogram
        """

EqualWidthMethods = Manual | SquareRoot | Sturges | Rice | Doane | Scott | TerrellScott | FreedmanDiaconis

class EqualWidthBinning:
    def __init__(self, method: EqualWidthMethods = Doane()):
        """Initialize the equal-width binning configuration.

        This strategy divides the range of values into bins of equal width.
        Several binning rules are supported to automatically determine the
        appropriate number of bins based on the input distribution.

        Note:
            Detailed explanations of each method are provided in their respective
            constructors or documentation.

        Args:
            method:
                Specifies how the number of bins should be determined.
                Options include:
                  - Manual(num_bins): Explicitly sets the number of bins.
                  - SquareRoot, Sturges, Rice, Doane, Scott, TerrellScott,
                    FreedmanDiaconis: Rules that infer bin counts from data.
                Defaults to Doane().
        """

    @property
    def method(self) -> EqualWidthMethods:
        """Specifies how the number of bins should be determined."""

    @method.setter
    def method(self, method: EqualWidthMethods) -> None:
        """Specifies how the number of bins should be determined."""

class QuantileBinning:
    def __init__(self, num_bins: int = 10):
        """Initialize the quantile binning strategy.

        This strategy uses the R-7 quantile method (Hyndman & Fan Type 7) to
        compute bin edges. It is the default quantile method in R and provides
        continuous, median-unbiased estimates that are approximately unbiased
        for normal distributions.

        The R-7 method defines quantiles using:
            - m = 1 - p
            - j = floor(n * p + m)
            - h = n * p + m - j
            - Q(p) = (1 - h) * x[j] + h * x[j+1]

        Reference:
            Hyndman, R. J. & Fan, Y. (1996). "Sample quantiles in statistical packages."
            The American Statistician, 50(4), pp. 361–365.
            PDF: https://www.amherst.edu/media/view/129116/original/Sample+Quantiles.pdf

        Args:
            num_bins:
                Number of bins to compute using the R-7 quantile method.
        """

    @property
    def num_bins(self) -> int:
        """The number of bins you want created using the r7 quantile method"""

    @num_bins.setter
    def num_bins(self, num_bins: int) -> None:
        """Set the number of bins you want created using the r7 quantile method"""

class PsiDriftConfig:
    def __init__(
        self,
        space: str = "__missing__",
        name: str = "__missing__",
        version: str = "0.1.0",
        alert_config: PsiAlertConfig = PsiAlertConfig(),
        config_path: Optional[Path] = None,
        categorical_features: Optional[list[str]] = None,
        binning_strategy: QuantileBinning | EqualWidthBinning = QuantileBinning(num_bins=10),
    ):
        """Initialize monitor config

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version. Defaults to 0.1.0
            alert_config:
                Alert configuration
            config_path:
                Optional path to load config from.
            categorical_features:
                List of features to treat as categorical for PSI calculation.
            binning_strategy:
                Strategy for binning continuous features during PSI calculation.
                Supports:
                  - QuantileBinning (R-7 method, Hyndman & Fan Type 7).
                  - EqualWidthBinning which divides the range of values into fixed-width bins.
                Default is QuantileBinning with 10 bins. You can also specify methods like Doane's rule with EqualWidthBinning.
        """

    @property
    def name(self) -> str:
        """Model Name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set model name"""

    @property
    def space(self) -> str:
        """Model space"""

    @space.setter
    def space(self, space: str) -> None:
        """Set model space"""

    @property
    def version(self) -> str:
        """Model version"""

    @version.setter
    def version(self, version: str) -> None:
        """Set model version"""

    @property
    def uid(self) -> str:
        """Unique identifier for the drift config"""

    @uid.setter
    def uid(self, uid: str) -> None:
        """Set unique identifier for the drift config"""

    @property
    def feature_map(self) -> Optional[FeatureMap]:
        """Feature map"""

    @property
    def alert_config(self) -> PsiAlertConfig:
        """Alert configuration"""

    @alert_config.setter
    def alert_config(self, alert_config: PsiAlertConfig) -> None:
        """Set alert configuration"""

    @property
    def drift_type(self) -> DriftType:
        """Drift type"""

    @property
    def binning_strategy(self) -> QuantileBinning | EqualWidthBinning:
        """binning_strategy"""

    @binning_strategy.setter
    def binning_strategy(self, binning_strategy: QuantileBinning | EqualWidthBinning) -> None:
        """Set binning_strategy"""

    @property
    def categorical_features(self) -> list[str]:
        """list of categorical features"""

    @categorical_features.setter
    def categorical_features(self, categorical_features: list[str]) -> None:
        """Set list of categorical features"""

    @staticmethod
    def load_from_json_file(path: Path) -> "PsiDriftConfig":
        """Load config from json file

        Args:
            path:
                Path to json file to load config from.
        """

    def __str__(self) -> str:
        """Return the string representation of the config."""

    def model_dump_json(self) -> str:
        """Return the json representation of the config."""

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        alert_config: Optional[PsiAlertConfig] = None,
        categorical_features: Optional[list[str]] = None,
        binning_strategy: Optional[QuantileBinning | EqualWidthBinning] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            alert_config:
                Alert configuration
            categorical_features:
                Categorical features
            binning_strategy:
                Binning strategy
        """

class PsiDriftProfile:
    @property
    def uid(self) -> str:
        """Return the unique identifier for the drift profile"""

    @property
    def scouter_version(self) -> str:
        """Return scouter version used to create DriftProfile"""

    @property
    def features(self) -> Dict[str, PsiFeatureDriftProfile]:
        """Return the list of features."""

    @property
    def config(self) -> PsiDriftConfig:
        """Return the monitor config."""

    def model_dump_json(self) -> str:
        """Return json representation of drift profile"""

    def model_dump(self) -> Dict[str, Any]:
        """Return dictionary representation of drift profile"""

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift profile to json file

        Args:
            path:
                Optional path to save the drift profile. If None, outputs to `psi_drift_profile.json`

        Returns:
            Path to the saved json file
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "PsiDriftProfile":
        """Load drift profile from json

        Args:
            json_string:
                JSON string representation of the drift profile

        """

    @staticmethod
    def from_file(path: Path) -> "PsiDriftProfile":
        """Load drift profile from file

        Args:
            path: Path to the file
        """

    @staticmethod
    def model_validate(data: Dict[str, Any]) -> "PsiDriftProfile":
        """Load drift profile from dictionary

        Args:
            data:
                DriftProfile dictionary
        """

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        alert_config: Optional[PsiAlertConfig] = None,
        categorical_features: Optional[list[str]] = None,
        binning_strategy: Optional[QuantileBinning | EqualWidthBinning] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            alert_config:
                Alert configuration
            categorical_features:
                Categorical Features
            binning_strategy:
                Binning strategy
        """

    def __str__(self) -> str:
        """Sting representation of DriftProfile"""

class Bin:
    @property
    def id(self) -> int:
        """Return the bin id."""

    @property
    def lower_limit(self) -> float:
        """Return the lower limit of the bin."""

    @property
    def upper_limit(self) -> Optional[float]:
        """Return the upper limit of the bin."""

    @property
    def proportion(self) -> float:
        """Return the proportion of data found in the bin."""

class BinType:
    Numeric: "BinType"
    Category: "BinType"

class PsiFeatureDriftProfile:
    @property
    def id(self) -> str:
        """Return the feature name"""

    @property
    def bins(self) -> List[Bin]:
        """Return the bins"""

    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

    @property
    def bin_type(self) -> BinType:
        """Return the timestamp."""

class PsiDriftMap:
    """Drift map of features"""

    @property
    def space(self) -> str:
        """Space to associate with drift map"""

    @property
    def name(self) -> str:
        """name to associate with drift map"""

    @property
    def version(self) -> str:
        """Version to associate with drift map"""

    @property
    def features(self) -> Dict[str, float]:
        """Returns dictionary of features and their reported drift, if any"""

    def __str__(self) -> str:
        """Return string representation of data drift"""

    def model_dump_json(self) -> str:
        """Return json representation of data drift"""

    @staticmethod
    def model_validate_json(json_string: str) -> "PsiDriftMap":
        """Load drift map from json file.

        Args:
            json_string:
                JSON string representation of the drift map
        """

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift map to json file

        Args:
            path:
                Optional path to save the drift map. If None, outputs to `psi_drift_map.json`

        Returns:
            Path to the saved json file

        """

class LLMDriftMap:
    @property
    def records(self) -> List[LLMMetricRecord]:
        """Return the list of LLM records."""

    def __str__(self): ...

class CustomMetricDriftConfig:
    def __init__(
        self,
        space: str = "__missing__",
        name: str = "__missing__",
        version: str = "0.1.0",
        sample_size: int = 25,
        alert_config: CustomMetricAlertConfig = CustomMetricAlertConfig(),
    ):
        """Initialize drift config
        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version. Defaults to 0.1.0
            sample_size:
                Sample size
            alert_config:
                Custom metric alert configuration
        """

    @property
    def space(self) -> str:
        """Model space"""

    @space.setter
    def space(self, space: str) -> None:
        """Set model space"""

    @property
    def name(self) -> str:
        """Model Name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set model name"""

    @property
    def version(self) -> str:
        """Model version"""

    @version.setter
    def version(self, version: str) -> None:
        """Set model version"""

    @property
    def uid(self) -> str:
        """Unique identifier for the drift config"""

    @uid.setter
    def uid(self, uid: str) -> None:
        """Set unique identifier for the drift config"""

    @property
    def drift_type(self) -> DriftType:
        """Drift type"""

    @property
    def alert_config(self) -> CustomMetricAlertConfig:
        """get alert_config"""

    @alert_config.setter
    def alert_config(self, alert_config: CustomMetricAlertConfig) -> None:
        """Set alert_config"""

    @staticmethod
    def load_from_json_file(path: Path) -> "CustomMetricDriftConfig":
        """Load config from json file
        Args:
            path:
                Path to json file to load config from.
        """

    def __str__(self) -> str:
        """Return the string representation of the config."""

    def model_dump_json(self) -> str:
        """Return the json representation of the config."""

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        alert_config: Optional[CustomMetricAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args
        Args:
            space:
                Model space
            name:
                Model name
            version:
                Model version
            alert_config:
                Custom metric alert configuration
        """

class CustomMetric:
    def __init__(
        self,
        name: str,
        value: float,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Optional[float] = None,
    ):
        """
        Initialize a custom metric for alerting.

        This class represents a custom metric that uses comparison-based alerting. It applies
        an alert condition to a single metric value.

        Args:
            name (str): The name of the metric being monitored. This should be a
                descriptive identifier for the metric.
            value (float): The current value of the metric.
            alert_threshold (AlertThreshold):
                The condition used to determine when an alert should be triggered.
            alert_threshold_value (Optional[float]):
                The threshold or boundary value used in conjunction with the alert_threshold.
                If supplied, this value will be added or subtracted from the provided metric value to
                determine if an alert should be triggered.

        """

    @property
    def name(self) -> str:
        """Return the metric name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set the metric name"""

    @property
    def value(self) -> float:
        """Return the metric value"""

    @value.setter
    def value(self, value: float) -> None:
        """Set the metric value"""

    @property
    def alert_condition(self) -> CustomMetricAlertCondition:
        """Return the alert_condition"""

    @alert_condition.setter
    def alert_condition(self, alert_condition: CustomMetricAlertCondition) -> None:
        """Set the alert_condition"""

    @property
    def alert_threshold(self) -> AlertThreshold:
        """Return the alert_threshold"""

    @property
    def alert_threshold_value(self) -> Optional[float]:
        """Return the alert_threshold_value"""

    def __str__(self) -> str:
        """Return the string representation of the config."""

class CustomDriftProfile:
    def __init__(
        self,
        config: CustomMetricDriftConfig,
        metrics: list[CustomMetric],
    ):
        """Initialize a CustomDriftProfile instance.

        Args:
            config (CustomMetricDriftConfig):
                The configuration for custom metric drift detection.
            metrics (list[CustomMetric]):
                A list of CustomMetric objects representing the metrics to be monitored.

        Example:
            config = CustomMetricDriftConfig(...)
            metrics = [CustomMetric("accuracy", 0.95), CustomMetric("f1_score", 0.88)]
            profile = CustomDriftProfile(config, metrics, "1.0.0")
        """

    @property
    def uid(self) -> str:
        """Return the unique identifier for the drift profile"""

    @property
    def config(self) -> CustomMetricDriftConfig:
        """Return the drift config"""

    @property
    def metrics(self) -> dict[str, float]:
        """Return custom metrics and their corresponding values"""

    @property
    def scouter_version(self) -> str:
        """Return scouter version used to create DriftProfile"""

    @property
    def custom_metrics(self) -> list[CustomMetric]:
        """Return custom metric objects that were used to create the drift profile"""

    def __str__(self) -> str:
        """Sting representation of DriftProfile"""

    def model_dump_json(self) -> str:
        """Return json representation of drift profile"""

    def model_dump(self) -> Dict[str, Any]:
        """Return dictionary representation of drift profile"""

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift profile to json file

        Args:
            path:
                Optional path to save the drift profile. If None, outputs to `custom_drift_profile.json`

        Returns:
            Path to the saved json file
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "CustomDriftProfile":
        """Load drift profile from json

        Args:
            json_string:
                JSON string representation of the drift profile

        """

    @staticmethod
    def model_validate(data: Dict[str, Any]) -> "CustomDriftProfile":
        """Load drift profile from dictionary

        Args:
            data:
                DriftProfile dictionary
        """

    @staticmethod
    def from_file(path: Path) -> "CustomDriftProfile":
        """Load drift profile from file

        Args:
            path: Path to the file
        """

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        alert_config: Optional[CustomMetricAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            space (Optional[str]):
                Model space
            name (Optional[str]):
                Model name
            version (Optional[str]):
                Model version
            alert_config (Optional[CustomMetricAlertConfig]):
                Custom metric alert configuration

        Returns:
            None
        """

class LLMDriftMetric:
    """Metric for monitoring LLM performance."""

    def __init__(
        self,
        name: str,
        value: float,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Optional[float] = None,
        prompt: Optional[Prompt] = None,
    ):
        """
        Initialize a metric for monitoring LLM performance.

        Args:
            name (str):
                The name of the metric being monitored. This should be a
                descriptive identifier for the metric.
            value (float):
                The current value of the metric.
            alert_threshold (AlertThreshold):
                The condition used to determine when an alert should be triggered.
            alert_threshold_value (Optional[float]):
                The threshold or boundary value used in conjunction with the alert_threshold.
                If supplied, this value will be added or subtracted from the provided metric value to
                determine if an alert should be triggered.
            prompt (Optional[Prompt]):
                Optional prompt associated with the metric. This can be used to provide context or
                additional information about the metric being monitored. If creating an LLM drift profile
                from a pre-defined workflow, this can be none.
        """

    @property
    def name(self) -> str:
        """Return the metric name"""

    @property
    def value(self) -> float:
        """Return the metric value"""

    @property
    def prompt(self) -> Optional[Prompt]:
        """Return the prompt associated with the metric"""

    @property
    def alert_threshold(self) -> AlertThreshold:
        """Return the alert_threshold"""

    @property
    def alert_threshold_value(self) -> Optional[float]:
        """Return the alert_threshold_value"""

class LLMMetricRecord:
    @property
    def uid(self) -> str:
        """Return the record uid"""

    @property
    def entity_uid(self) -> str:
        """Returns the entity uid associated with the record"""

    @property
    def created_at(self) -> datetime:
        """Return the timestamp when the record was created"""

    @property
    def metric(self) -> str:
        """Return the name of the metric associated with the record"""

    @property
    def value(self) -> float:
        """Return the value of the metric associated with the record"""

    def __str__(self) -> str:
        """Return the string representation of the record"""

class LLMDriftConfig:
    def __init__(
        self,
        space: str = "__missing__",
        name: str = "__missing__",
        version: str = "0.1.0",
        sample_rate: int = 5,
        alert_config: LLMAlertConfig = LLMAlertConfig(),
    ):
        """Initialize drift config
        Args:
            space:
                Space to associate with the config
            name:
                Name to associate with the config
            version:
                Version to associate with the config. Defaults to 0.1.0
            sample_rate:
                Sample rate for LLM drift detection. Defaults to 5.
            alert_config:
                Custom metric alert configuration
        """

    @property
    def space(self) -> str:
        """Model space"""

    @space.setter
    def space(self, space: str) -> None:
        """Set model space"""

    @property
    def name(self) -> str:
        """Model Name"""

    @name.setter
    def name(self, name: str) -> None:
        """Set model name"""

    @property
    def version(self) -> str:
        """Model version"""

    @version.setter
    def version(self, version: str) -> None:
        """Set model version"""

    @property
    def uid(self) -> str:
        """Unique identifier for the drift config"""

    @uid.setter
    def uid(self, uid: str) -> None:
        """Set unique identifier for the drift config"""

    @property
    def drift_type(self) -> DriftType:
        """Drift type"""

    @property
    def alert_config(self) -> LLMAlertConfig:
        """get alert_config"""

    @alert_config.setter
    def alert_config(self, alert_config: LLMAlertConfig) -> None:
        """Set alert_config"""

    @staticmethod
    def load_from_json_file(path: Path) -> "LLMDriftConfig":
        """Load config from json file
        Args:
            path:
                Path to json file to load config from.
        """

    def __str__(self) -> str:
        """Return the string representation of the config."""

    def model_dump_json(self) -> str:
        """Return the json representation of the config."""

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        alert_config: Optional[LLMAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args
        Args:
            space:
                Space to associate with the config
            name:
                Name to associate with the config
            version:
                Version to associate with the config
            alert_config:
                LLM alert configuration
        """

class LLMDriftProfile:
    def __init__(
        self,
        config: LLMDriftConfig,
        metrics: list[LLMDriftMetric],
        workflow: Optional[Workflow] = None,
    ):
        """Initialize a LLMDriftProfile for LLM evaluation and drift detection.

        LLM evaluations are run asynchronously on the scouter server.

        Logic flow:
            1. If only metrics are provided, a workflow will be created automatically
               from the metrics. In this case a prompt is required for each metric.
            2. If a workflow is provided, it will be parsed and validated for compatibility:
               - A list of metrics to evaluate workflow output must be provided
               - Metric names must correspond to the final task names in the workflow

        Baseline metrics and thresholds will be extracted from the LLMDriftMetric objects.

        Args:
            config (LLMDriftConfig):
                The configuration for the LLM drift profile containing space, name,
                version, and alert settings.
            metrics (list[LLMDriftMetric]):
                A list of LLMDriftMetric objects representing the metrics to be monitored.
                Each metric defines evaluation criteria and alert thresholds.
            workflow (Optional[Workflow]):
                Optional custom workflow for advanced evaluation scenarios. If provided,
                the workflow will be validated to ensure proper parameter and response
                type configuration.

        Returns:
            LLMDriftProfile: Configured profile ready for LLM drift monitoring.

        Raises:
            ProfileError: If workflow validation fails, metrics are empty when no
                workflow is provided, or if workflow tasks don't match metric names.

        Examples:
            Basic usage with metrics only:

            >>> config = LLMDriftConfig("my_space", "my_model", "1.0")
            >>> metrics = [
            ...     LLMDriftMetric("accuracy", 0.95, AlertThreshold.Above, 0.1, prompt),
            ...     LLMDriftMetric("relevance", 0.85, AlertThreshold.Below, 0.2, prompt2)
            ... ]
            >>> profile = LLMDriftProfile(config, metrics)

            Advanced usage with custom workflow:

            >>> workflow = create_custom_workflow()  # Your custom workflow
            >>> metrics = [LLMDriftMetric("final_task", 0.9, AlertThreshold.Above)]
            >>> profile = LLMDriftProfile(config, metrics, workflow)

        Note:
            - When using custom workflows, ensure final tasks have Score response types
            - Initial workflow tasks must include "input" and/or "response" parameters
            - All metric names must match corresponding workflow task names
        """

    @property
    def uid(self) -> str:
        """Return the unique identifier for the drift profile"""

    @property
    def config(self) -> LLMDriftConfig:
        """Return the drift config"""

    @property
    def metrics(self) -> List[LLMDriftMetric]:
        """Return LLM metrics and their corresponding values"""

    @property
    def scouter_version(self) -> str:
        """Return scouter version used to create DriftProfile"""

    def __str__(self) -> str:
        """String representation of LLMDriftProfile"""

    def model_dump_json(self) -> str:
        """Return json representation of drift profile"""

    def model_dump(self) -> Dict[str, Any]:
        """Return dictionary representation of drift profile"""

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save drift profile to json file

        Args:
            path: Optional path to save the json file. If not provided, a default path will be used.

        Returns:
            Path to the saved json file.
        """

    @staticmethod
    def model_validate(data: Dict[str, Any]) -> "LLMDriftProfile":
        """Load drift profile from dictionary

        Args:
            data:
                DriftProfile dictionary
        """

    @staticmethod
    def model_validate_json(json_string: str) -> "LLMDriftProfile":
        """Load drift profile from json

        Args:
            json_string:
                JSON string representation of the drift profile
        """

    @staticmethod
    def from_file(path: Path) -> "LLMDriftProfile":
        """Load drift profile from file

        Args:
            path: Path to the json file

        Returns:
            LLMDriftProfile
        """

    def update_config_args(
        self,
        space: Optional[str] = None,
        name: Optional[str] = None,
        version: Optional[str] = None,
        sample_size: Optional[int] = None,
        alert_config: Optional[LLMAlertConfig] = None,
    ) -> None:
        """Inplace operation that updates config args

        Args:
            name:
                Model name
            space:
                Model space
            version:
                Model version
            sample_size:
                Sample size
            alert_config:
                Alert configuration
        """

class Drifter:
    def __init__(self) -> None:
        """Instantiate Rust Drifter class that is
        used to create monitoring profiles and compute drifts.
        """

    @overload
    def create_drift_profile(
        self,
        data: Any,
        config: SpcDriftConfig,
        data_type: Optional[ScouterDataType] = None,
    ) -> SpcDriftProfile:
        """Create a SPC (Statistical process control) drift profile from the provided data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.

                **Data is expected to not contain any missing values, NaNs or infinities**

            config:
                SpcDriftConfig
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            SpcDriftProfile
        """

    @overload
    def create_drift_profile(
        self,
        data: Any,
        data_type: Optional[ScouterDataType] = None,
    ) -> SpcDriftProfile:
        """Create a SPC (Statistical process control) drift profile from the provided data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.

                **Data is expected to not contain any missing values, NaNs or infinities**

            config:
                SpcDriftConfig
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            SpcDriftProfile
        """

    @overload
    def create_drift_profile(
        self,
        data: Any,
        config: PsiDriftConfig,
        data_type: Optional[ScouterDataType] = None,
    ) -> PsiDriftProfile:
        """Create a PSI (population stability index) drift profile from the provided data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.

                **Data is expected to not contain any missing values, NaNs or infinities**

            config:
                PsiDriftConfig
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            PsiDriftProfile
        """

    @overload
    def create_drift_profile(
        self,
        data: Union[CustomMetric, List[CustomMetric]],
        config: CustomMetricDriftConfig,
        data_type: Optional[ScouterDataType] = None,
    ) -> CustomDriftProfile:
        """Create a custom drift profile from data.

        Args:
            data:
                CustomMetric or list of CustomMetric.
            config:
                CustomMetricDriftConfig
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            CustomDriftProfile
        """

    def create_drift_profile(  # type: ignore
        self,
        data: Any,
        config: Optional[Union[SpcDriftConfig, PsiDriftConfig, CustomMetricDriftConfig]] = None,
        data_type: Optional[ScouterDataType] = None,
    ) -> Union[SpcDriftProfile, PsiDriftProfile, CustomDriftProfile]:
        """Create a drift profile from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe, pandas dataframe or a list of CustomMetric if creating
                a custom metric profile.

                **Data is expected to not contain any missing values, NaNs or infinities**

            config:
                Drift config that will be used for monitoring
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            SpcDriftProfile, PsiDriftProfile or CustomDriftProfile
        """

    def create_llm_drift_profile(
        self,
        config: LLMDriftConfig,
        metrics: List[LLMDriftMetric],
        workflow: Optional[Workflow] = None,
    ) -> LLMDriftProfile:
        """Initialize a LLMDriftProfile for LLM evaluation and drift detection.

        LLM evaluations are run asynchronously on the scouter server.

        Logic flow:
            1. If only metrics are provided, a workflow will be created automatically
               from the metrics. In this case a prompt is required for each metric.
            2. If a workflow is provided, it will be parsed and validated for compatibility:
               - A list of metrics to evaluate workflow output must be provided
               - Metric names must correspond to the final task names in the workflow

        Baseline metrics and thresholds will be extracted from the LLMDriftMetric objects.

        Args:
            config (LLMDriftConfig):
                The configuration for the LLM drift profile containing space, name,
                version, and alert settings.
            metrics (list[LLMDriftMetric]):
                A list of LLMDriftMetric objects representing the metrics to be monitored.
                Each metric defines evaluation criteria and alert thresholds.
            workflow (Optional[Workflow]):
                Optional custom workflow for advanced evaluation scenarios. If provided,
                the workflow will be validated to ensure proper parameter and response
                type configuration.

        Returns:
            LLMDriftProfile: Configured profile ready for LLM drift monitoring.

        Raises:
            ProfileError: If workflow validation fails, metrics are empty when no
                workflow is provided, or if workflow tasks don't match metric names.

        Examples:
            Basic usage with metrics only:

            >>> config = LLMDriftConfig("my_space", "my_model", "1.0")
            >>> metrics = [
            ...     LLMDriftMetric("accuracy", 0.95, AlertThreshold.Above, 0.1, prompt),
            ...     LLMDriftMetric("relevance", 0.85, AlertThreshold.Below, 0.2, prompt2)
            ... ]
            >>> profile = Drifter().create_llm_drift_profile(config, metrics)

            Advanced usage with custom workflow:

            >>> workflow = create_custom_workflow()  # Your custom workflow
            >>> metrics = [LLMDriftMetric("final_task", 0.9, AlertThreshold.Above)]
            >>> profile = Drifter().create_llm_drift_profile(config, metrics, workflow)

        Note:
            - When using custom workflows, ensure final tasks have Score response types
            - Initial workflow tasks must include "input" and/or "response" parameters
            - All metric names must match corresponding workflow task names
        """

    @overload
    def compute_drift(
        self,
        data: Any,
        drift_profile: SpcDriftProfile,
        data_type: Optional[ScouterDataType] = None,
    ) -> SpcDriftMap:
        """Create a drift map from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.
            drift_profile:
                Drift profile to use to compute drift map
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            SpcDriftMap
        """

    @overload
    def compute_drift(
        self,
        data: Any,
        drift_profile: PsiDriftProfile,
        data_type: Optional[ScouterDataType] = None,
    ) -> PsiDriftMap:
        """Create a drift map from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.
            drift_profile:
                Drift profile to use to compute drift map
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            PsiDriftMap
        """

    @overload
    def compute_drift(
        self,
        data: Union[LLMRecord, List[LLMRecord]],
        drift_profile: LLMDriftProfile,
        data_type: Optional[ScouterDataType] = None,
    ) -> LLMDriftMap:
        """Create a drift map from data.

        Args:
            data:

            drift_profile:
                Drift profile to use to compute drift map
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            LLMDriftMap
        """

    def compute_drift(  # type: ignore
        self,
        data: Any,
        drift_profile: Union[SpcDriftProfile, PsiDriftProfile, LLMDriftProfile],
        data_type: Optional[ScouterDataType] = None,
    ) -> Union[SpcDriftMap, PsiDriftMap, LLMDriftMap]:
        """Create a drift map from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or a pandas dataframe.
            drift_profile:
                Drift profile to use to compute drift map
            data_type:
                Optional data type. Inferred from data if not provided.

        Returns:
            SpcDriftMap, PsiDriftMap or LLMDriftMap
        """

class LLMEvalTaskResult:
    """Eval Result for a specific evaluation"""

    @property
    def id(self) -> str:
        """Get the record id associated with this result"""

    @property
    def metrics(self) -> Dict[str, Score]:
        """Get the list of metrics"""

    @property
    def embedding(self) -> Dict[str, List[float]]:
        """Get embeddings of embedding targets"""

class LLMEvalResults:
    """Defines the results of an LLM eval metric"""

    def __getitem__(self, key: str) -> LLMEvalTaskResult:
        """Get the task results for a specific record ID. A RuntimeError will be raised if the record ID does not exist."""

    def __str__(self):
        """String representation of the LLMEvalResults"""

    def to_dataframe(self, polars: bool = False) -> Any:
        """
        Convert the results to a Pandas or Polars DataFrame.

        Args:
            polars (bool):
                Whether to return a Polars DataFrame. If False, a Pandas DataFrame will be returned.

        Returns:
            DataFrame:
                A Pandas or Polars DataFrame containing the results.
        """

    def model_dump_json(self) -> str:
        """Dump the results as a JSON string"""

    @staticmethod
    def model_validate_json(json_string: str) -> "LLMEvalResults":
        """Validate and create an LLMEvalResults instance from a JSON string

        Args:
            json_string (str):
                JSON string to validate and create the LLMEvalResults instance from.
        """

    @property
    def errored_tasks(self) -> List[str]:
        """Get a list of record IDs that had errors during evaluation"""

    @property
    def histograms(self) -> Optional[Dict[str, Histogram]]:
        """Get histograms for all calculated features (metrics, embeddings, similarities)"""

class LLMEvalMetric:
    """Defines an LLM eval metric to use when evaluating LLMs"""

    def __init__(self, name: str, prompt: Prompt):
        """
        Initialize an LLMEvalMetric to use for evaluating LLMs. This is
        most commonly used in conjunction with `evaluate_llm` where LLM inputs
        and responses can be evaluated against a variety of user-defined metrics.

        Args:
            name (str):
                Name of the metric
            prompt (Prompt):
                Prompt to use for the metric. For example, a user may create
                an accuracy analysis prompt or a query reformulation analysis prompt.
        """

    def __str__(self) -> str:
        """
        String representation of the LLMEvalMetric
        """

class LLMEvalRecord:
    """LLM record containing context tied to a Large Language Model interaction
    that is used to evaluate LLM responses.


    Examples:
        >>> record = LLMEvalRecord(
                id="123",
                context={
                    "input": "What is the capital of France?",
                    "response": "Paris is the capital of France."
                },
        ... )
        >>> print(record.context["input"])
        "What is the capital of France?"
    """

    def __init__(
        self,
        context: Context,
        id: Optional[str] = None,
    ) -> None:
        """Creates a new LLM record to associate with an `LLMDriftProfile`.
        The record is sent to the `Scouter` server via the `ScouterQueue` and is
        then used to inject context into the evaluation prompts.

        Args:
            context:
                Additional context information as a dictionary or a pydantic BaseModel. During evaluation,
                this will be merged with the input and response data and passed to the assigned
                evaluation prompts. So if you're evaluation prompts expect additional context via
                bound variables (e.g., `${foo}`), you can pass that here as key value pairs.
                {"foo": "bar"}
            id:
                Unique identifier for the record. If not provided, a new UUID will be generated.
                This is helpful for when joining evaluation results back to the original request.

        Raises:
            TypeError: If context is not a dict or a pydantic BaseModel.

        """

    @property
    def context(self) -> Dict[str, Any]:
        """Get the contextual information.

        Returns:
            The context data as a Python object (deserialized from JSON).
        """

def evaluate_llm(
    records: List[LLMEvalRecord],
    metrics: List[LLMEvalMetric],
    config: Optional[EvaluationConfig] = None,
) -> LLMEvalResults:
    """
    Evaluate LLM responses using the provided evaluation metrics.

    Args:
        records (List[LLMEvalRecord]):
            List of LLM evaluation records to evaluate.
        metrics (List[LLMEvalMetric]):
            List of LLMEvalMetric instances to use for evaluation.
        config (Optional[EvaluationConfig]):
            Optional EvaluationConfig instance to configure evaluation options.

    Returns:
        LLMEvalResults
    """

class EvaluationConfig:
    """Configuration options for LLM evaluation."""

    def __init__(
        self,
        embedder: Optional[Embedder] = None,
        embedding_targets: Optional[List[str]] = None,
        compute_similarity: bool = False,
        cluster: bool = False,
        compute_histograms: bool = False,
    ):
        """
        Initialize the EvaluationConfig with optional parameters.

        Args:
            embedder (Optional[Embedder]):
                Optional Embedder instance to use for generating embeddings for similarity-based metrics.
                If not provided, no embeddings will be generated.
            embedding_targets (Optional[List[str]]):
                Optional list of context keys to generate embeddings for. If not provided, embeddings will
                be generated for all string fields in the record context.
            compute_similarity (bool):
                Whether to compute similarity between embeddings. Default is False.
            cluster (bool):
                Whether to perform clustering on the embeddings. Default is False.
            compute_histograms (bool):
                Whether to compute histograms for all calculated features (metrics, embeddings, similarities).
                Default is False.
        """

########
# __scouter.profile__
########

class Distinct:
    @property
    def count(self) -> int:
        """total unique value counts"""

    @property
    def percent(self) -> float:
        """percent value uniqueness"""

class Quantiles:
    @property
    def q25(self) -> float:
        """25th quantile"""

    @property
    def q50(self) -> float:
        """50th quantile"""

    @property
    def q75(self) -> float:
        """75th quantile"""

    @property
    def q99(self) -> float:
        """99th quantile"""

class Histogram:
    @property
    def bins(self) -> List[float]:
        """Bin values"""

    @property
    def bin_counts(self) -> List[int]:
        """Bin counts"""

class NumericStats:
    @property
    def mean(self) -> float:
        """Return the mean."""

    @property
    def stddev(self) -> float:
        """Return the stddev."""

    @property
    def min(self) -> float:
        """Return the min."""

    @property
    def max(self) -> float:
        """Return the max."""

    @property
    def distinct(self) -> Distinct:
        """Distinct value counts"""

    @property
    def quantiles(self) -> Quantiles:
        """Value quantiles"""

    @property
    def histogram(self) -> Histogram:
        """Value histograms"""

class CharStats:
    @property
    def min_length(self) -> int:
        """Minimum string length"""

    @property
    def max_length(self) -> int:
        """Maximum string length"""

    @property
    def median_length(self) -> int:
        """Median string length"""

    @property
    def mean_length(self) -> float:
        """Mean string length"""

class WordStats:
    @property
    def words(self) -> Dict[str, Distinct]:
        """Distinct word counts"""

class StringStats:
    @property
    def distinct(self) -> Distinct:
        """Distinct value counts"""

    @property
    def char_stats(self) -> CharStats:
        """Character statistics"""

    @property
    def word_stats(self) -> WordStats:
        """word statistics"""

class FeatureProfile:
    @property
    def id(self) -> str:
        """Return the id."""

    @property
    def numeric_stats(self) -> Optional[NumericStats]:
        """Return the numeric stats."""

    @property
    def string_stats(self) -> Optional[StringStats]:
        """Return the string stats."""

    @property
    def timestamp(self) -> str:
        """Return the timestamp."""

    @property
    def correlations(self) -> Optional[Dict[str, float]]:
        """Feature correlation values"""

    def __str__(self) -> str:
        """Return the string representation of the feature profile."""

class DataProfile:
    """Data profile of features"""

    @property
    def features(self) -> Dict[str, FeatureProfile]:
        """Returns dictionary of features and their data profiles"""

    def __str__(self) -> str:
        """Return string representation of the data profile"""

    def model_dump_json(self) -> str:
        """Return json representation of data profile"""

    @staticmethod
    def model_validate_json(json_string: str) -> "DataProfile":
        """Load Data profile from json

        Args:
            json_string:
                JSON string representation of the data profile
        """

    def save_to_json(self, path: Optional[Path] = None) -> Path:
        """Save data profile to json file

        Args:
            path:
                Optional path to save the data profile. If None, outputs to `data_profile.json`

        Returns:
            Path to the saved data profile

        """

class DataProfiler:
    def __init__(self):
        """Instantiate DataProfiler class that is
        used to profile data"""

    def create_data_profile(
        self,
        data: Any,
        data_type: Optional[ScouterDataType] = None,
        bin_size: int = 20,
        compute_correlations: bool = False,
    ) -> DataProfile:
        """Create a data profile from data.

        Args:
            data:
                Data to create a data profile from. Data can be a numpy array,
                a polars dataframe or pandas dataframe.

                **Data is expected to not contain any missing values, NaNs or infinities**

                These types are incompatible with computing
                quantiles, histograms, and correlations. These values must be removed or imputed.

            data_type:
                Optional data type. Inferred from data if not provided.
            bin_size:
                Optional bin size for histograms. Defaults to 20 bins.
            compute_correlations:
                Whether to compute correlations or not.

        Returns:
            DataProfile
        """

    class TraceMetricsRequest:
        """Request to get trace metrics from the Scouter server."""

        def __init__(
            self,
            service_name: str,
            start_time: datetime,
            end_time: datetime,
            bucket_interval: str,
        ):
            """
            Initialize a TraceMetricsRequest.

            Args:
                service_name (str):
                    The name of the service to query metrics for.
                start_time (datetime):
                    The start time for the metrics query.
                end_time (datetime):
                    The end time for the metrics query.
                bucket_interval (str):
                    Optional interval for aggregating metrics (e.g., "1m", "5m").
            """

__all__ = [
    # alert
    "AlertZone",
    "SpcAlertType",
    "SpcAlertRule",
    "PsiAlertConfig",
    "PsiAlertConfig",
    "SpcAlertConfig",
    "SpcAlert",
    "AlertThreshold",
    "CustomMetricAlertCondition",
    "CustomMetricAlertConfig",
    "SlackDispatchConfig",
    "OpsGenieDispatchConfig",
    "ConsoleDispatchConfig",
    "AlertDispatchType",
    "PsiNormalThreshold",
    "PsiChiSquareThreshold",
    "PsiFixedThreshold",
    "LLMMetricAlertCondition",
    "LLMAlertConfig",
    # client
    "TimeInterval",
    "DriftRequest",
    "ScouterClient",
    "BinnedMetricStats",
    "BinnedMetric",
    "BinnedMetrics",
    "BinnedPsiMetric",
    "BinnedPsiFeatureMetrics",
    "SpcDriftFeature",
    "BinnedSpcFeatureMetrics",
    "ProfileStatusRequest",
    "Alert",
    "DriftAlertPaginationRequest",
    "DriftAlertPaginationResponse",
    "GetProfileRequest",
    "Attribute",
    "SpanEvent",
    "SpanLink",
    "TraceBaggageRecord",
    "TraceFilters",
    "TraceMetricBucket",
    "TraceListItem",
    "TraceSpan",
    "TracePaginationResponse",
    "TraceSpansResponse",
    "TraceBaggageResponse",
    "TraceMetricsRequest",
    "TraceMetricsResponse",
    "TagsResponse",
    "TagRecord",
    # drift
    "FeatureMap",
    "SpcFeatureDriftProfile",
    "SpcDriftConfig",
    "SpcDriftProfile",
    "SpcFeatureDrift",
    "SpcDriftMap",
    "PsiDriftConfig",
    "PsiDriftProfile",
    "PsiDriftMap",
    "CustomMetricDriftConfig",
    "CustomMetric",
    "CustomDriftProfile",
    "LLMDriftMetric",
    "LLMDriftConfig",
    "LLMDriftProfile",
    "Drifter",
    "QuantileBinning",
    "EqualWidthBinning",
    "Manual",
    "SquareRoot",
    "Sturges",
    "Rice",
    "Doane",
    "Scott",
    "TerrellScott",
    "FreedmanDiaconis",
    # evaluate
    "LLMEvalTaskResult",
    "LLMEvalMetric",
    "LLMEvalResults",
    "LLMEvalRecord",
    "evaluate_llm",
    "EvaluationConfig",
    # genai
    "PromptTokenDetails",
    "CompletionTokenDetails",
    "Usage",
    "ImageUrl",
    "AudioUrl",
    "BinaryContent",
    "DocumentUrl",
    "Message",
    "ModelSettings",
    "Prompt",
    "Provider",
    "TaskStatus",
    "AgentResponse",
    "Task",
    "TaskList",
    "Agent",
    "Workflow",
    "PyTask",
    "ChatResponse",
    "EventDetails",
    "TaskEvent",
    "WorkflowResult",
    "Score",
    "Embedder",
    # genai - google
    "Modality",
    "ThinkingConfig",
    "MediaResolution",
    "SpeechConfig",
    "PrebuiltVoiceConfig",
    "VoiceConfigMode",
    "VoiceConfig",
    "GenerationConfig",
    "ToolConfig",
    "FunctionCallingConfig",
    "RetrievalConfig",
    "LatLng",
    "ModelArmorConfig",
    "Mode",
    "GeminiSettings",
    "HarmCategory",
    "HarmBlockThreshold",
    "HarmBlockMethod",
    "SafetySetting",
    "GeminiEmbeddingConfig",
    "GeminiEmbeddingResponse",
    "PredictRequest",
    "PredictResponse",
    "EmbeddingTaskType",
    # openai
    "AudioParam",
    "ContentPart",
    "Content",
    "Prediction",
    "StreamOptions",
    "ToolChoiceMode",
    "FunctionChoice",
    "FunctionToolChoice",
    "CustomChoice",
    "CustomToolChoice",
    "ToolDefinition",
    "AllowedToolsMode",
    "AllowedTools",
    "ToolChoice",
    "FunctionDefinition",
    "FunctionTool",
    "TextFormat",
    "Grammar",
    "GrammarFormat",
    "CustomToolFormat",
    "CustomDefinition",
    "CustomTool",
    "Tool",
    "OpenAIChatSettings",
    "OpenAIEmbeddingConfig",
    "OpenAIEmbeddingResponse",
    # profile
    "Distinct",
    "Quantiles",
    "Histogram",
    "NumericStats",
    "CharStats",
    "WordStats",
    "StringStats",
    "FeatureProfile",
    "DataProfile",
    "DataProfiler",
    # queue
    "ScouterQueue",
    "Queue",
    "SpcRecord",
    "PsiRecord",
    "CustomMetricRecord",
    "ServerRecord",
    "ServerRecords",
    "QueueFeature",
    "Features",
    "RecordType",
    "Metric",
    "Metrics",
    "EntityType",
    "LLMRecord",
    # transport
    "HttpConfig",
    "KafkaConfig",
    "RabbitMQConfig",
    "RedisConfig",
    # types
    "DriftType",
    "CommonCrons",
    "ScouterDataType",
    # tracer
    "init_tracer",
    "SpanKind",
    "FunctionType",
    "ActiveSpan",
    "OtelExportConfig",
    "GrpcConfig",
    "GrpcSpanExporter",
    "HttpSpanExporter",
    "StdoutSpanExporter",
    "OtelProtocol",
    "TraceRecord",
    "TraceSpanRecord",
    "TraceBaggageRecord",
    "TestSpanExporter",
    "flush_tracer",
    "BatchConfig",
    "shutdown_tracer",
    "TraceMetricsRequest",
]
