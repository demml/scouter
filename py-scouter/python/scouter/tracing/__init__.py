# pylint: disable=dangerous-default-value,implicit-str-concat
# mypy: disable-error-code="attr-defined"

import functools
import threading
from contextlib import contextmanager
from typing import (
    TYPE_CHECKING,
    Any,
    AsyncGenerator,
    Awaitable,
    Callable,
    Collection,
    Generator,
    List,
    Mapping,
    Optional,
    ParamSpec,
    Sequence,
    TypeAlias,
    TypeVar,
    Union,
    cast,
)

from .._scouter import (
    ActiveSpan,
    BaseTracer,
    BatchConfig,
    FunctionType,
    GrpcSpanExporter,
    HttpSpanExporter,
    OtelExportConfig,
    OtelProtocol,
    SpanKind,
    StdoutSpanExporter,
    TestSpanExporter,
    TraceBaggageRecord,
    TraceRecord,
    TraceSpanRecord,
    disable_local_span_capture,
    drain_local_span_capture,
    enable_local_span_capture,
    extract_span_context_from_headers,
    flush_tracer,
    get_current_active_span,
    get_function_type,
    get_tracing_headers_from_current_span,
    init_tracer,
    shutdown_tracer,
)
from .middleware import ScouterTracingMiddleware

SerializedType: TypeAlias = Union[str, int, float, dict, list]
P = ParamSpec("P")
R = TypeVar("R")
SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY = "scouter.active.entity_uid"
HAS_OPENTELEMETRY = True
if TYPE_CHECKING:
    from opentelemetry.instrumentation.instrumentor import BaseInstrumentor
    from opentelemetry.trace import Tracer as _OtelTracer
    from opentelemetry.trace import TracerProvider as _OtelTracerProvider
    from opentelemetry.trace import set_tracer_provider
    from opentelemetry.util.types import Attributes

    from .._scouter import AgentEvalProfile
else:
    # Try to import OpenTelemetry, but provide fallbacks if not available
    try:
        from opentelemetry.instrumentation.instrumentor import BaseInstrumentor
        from opentelemetry.trace import Tracer as _OtelTracer
        from opentelemetry.trace import get_tracer_provider, set_tracer_provider

        HAS_OPENTELEMETRY = True
    except ImportError:
        HAS_OPENTELEMETRY = False

        # Provide stub base class when OpenTelemetry is not installed
        class BaseInstrumentor:
            """Stub base class when OpenTelemetry is not available."""

            def instrument(self, **kwargs):
                raise ImportError("OpenTelemetry is not installed. Install with: " "pip install opsml[opentelemetry]")

            def uninstrument(self, **kwargs):
                raise ImportError("OpenTelemetry is not installed. Install with: " "pip install opsml[opentelemetry]")

        def get_tracer_provider():
            raise ImportError("OpenTelemetry is not installed. Install with: " "pip install opsml[opentelemetry]")

        def set_tracer_provider(provider):
            raise ImportError("OpenTelemetry is not installed. Install with: " "pip install opsml[opentelemetry]")

    class _OtelTracerProvider:
        pass

    class _OtelTracer:
        pass

    AttributeValue = Union[
        str,
        bool,
        int,
        float,
        Sequence[str],
        Sequence[bool],
        Sequence[int],
        Sequence[float],
    ]

    Attributes = Optional[Mapping[str, AttributeValue]]


def set_output(
    span: ActiveSpan,
    outputs: List[Any],
    max_length: int,
    capture_last_stream_item: bool = False,
    join_stream_items: bool = False,
) -> None:
    """Helper to set output attribute on span with length check."""

    if capture_last_stream_item and outputs:
        span.set_output(outputs[-1], max_length)

    elif join_stream_items:
        span.set_output("".join(outputs), max_length)

    else:
        span.set_output(outputs, max_length)


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
        trace_id: Optional[str] = None,
        max_length: int = 1000,
        capture_last_stream_item: bool = False,
        join_stream_items: bool = False,
        **kwargs,
    ) -> Callable[[Callable[P, R]], Callable[P, R]]:
        """Decorator to trace function execution with OpenTelemetry spans.

        Args:
            name (Optional[str]):
                The name of the span. If None, defaults to the function name.
            kind (SpanKind):
                The kind of span (default: Internal)
            attributes (List[dict[str, str]]):
                Additional attributes to set on the span
            baggage (List[dict[str, str]]):
                Additional baggage to set on the span
            tags (List[dict[str, str]]):
                Additional tags to set on the span
            label (Optional[str]):
                An optional label for the span
            parent_context_id (Optional[str]):
                Parent context ID for the span
            trace_id (Optional[str]):
                Optional trace ID to associate with the span. This is useful for
            max_length (int):
                Maximum length for input/output capture
            capture_last_stream_item (bool):
                Whether to capture only the last item from streaming functions
            join_stream_items (bool):
                Whether to join all stream items into a single string for output
            **kwargs:
                Additional keyword arguments
        Returns:
            Callable[[Callable[P, R]], Callable[P, R]]:
        """

        # I'd prefer this entire decorator to be rust, but creating this type of decorator in rust
        # is a little bit of a pain when dealing with async
        def decorator(func: Callable[P, R]) -> Callable[P, R]:
            span_name = name or f"{func.__module__}.{getattr(func, '__qualname__', repr(func))}"
            function_type = get_function_type(func)

            if function_type == FunctionType.AsyncGenerator:

                @functools.wraps(func)
                async def async_generator_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    async with self._start_decorated_as_current_span(
                        name=span_name,
                        func=func,
                        func_args=args,
                        kind=kind,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        label=label,
                        parent_context_id=parent_context_id,
                        trace_id=trace_id,
                        max_length=max_length,
                        func_type=function_type,
                        func_kwargs=kwargs,
                    ) as span:
                        try:
                            async_gen_func = cast(Callable[P, AsyncGenerator[Any, None]], func)
                            generator = async_gen_func(*args, **kwargs)

                            outputs = []
                            async for item in generator:
                                outputs.append(item)
                                yield item

                            set_output(
                                span,
                                outputs,
                                max_length,
                                capture_last_stream_item,
                                join_stream_items,
                            )

                        except Exception as e:
                            span.set_attribute("error.type", type(e).__name__)
                            raise

                return cast(Callable[P, R], async_generator_wrapper)

            if function_type == FunctionType.SyncGenerator:

                @functools.wraps(func)
                def generator_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    with self._start_decorated_as_current_span(
                        name=span_name,
                        func=func,
                        func_args=args,
                        kind=kind,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        label=label,
                        parent_context_id=parent_context_id,
                        trace_id=trace_id,
                        max_length=max_length,
                        func_type=function_type,
                        func_kwargs=kwargs,
                    ) as span:
                        try:
                            gen_func = cast(Callable[P, Generator[Any, None, None]], func)
                            generator = gen_func(*args, **kwargs)
                            results = []

                            for item in generator:
                                results.append(item)
                                yield item

                            set_output(
                                span,
                                results,
                                max_length,
                                capture_last_stream_item,
                                join_stream_items,
                            )

                        except Exception as e:
                            span.set_attribute("error.type", type(e).__name__)
                            raise

                return cast(Callable[P, R], generator_wrapper)

            if function_type == FunctionType.Async:

                @functools.wraps(func)
                async def async_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    async with self._start_decorated_as_current_span(
                        name=span_name,
                        func=func,
                        func_args=args,
                        kind=kind,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        label=label,
                        parent_context_id=parent_context_id,
                        trace_id=trace_id,
                        max_length=max_length,
                        func_type=function_type,
                        func_kwargs=kwargs,
                    ) as span:
                        try:
                            async_func = cast(Callable[P, Awaitable[Any]], func)
                            result = await async_func(*args, **kwargs)

                            span.set_output(result, max_length)
                            return result

                        except Exception as e:
                            span.set_attribute("error.type", type(e).__name__)
                            raise

                return cast(Callable[P, R], async_wrapper)

            @functools.wraps(func)
            def sync_wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
                with self._start_decorated_as_current_span(
                    name=span_name,
                    func=func,
                    func_args=args,
                    kind=kind,
                    attributes=attributes,
                    baggage=baggage,
                    tags=tags,
                    label=label,
                    parent_context_id=parent_context_id,
                    trace_id=trace_id,
                    max_length=max_length,
                    func_type=function_type,
                    func_kwargs=kwargs,
                ) as span:
                    try:
                        result = func(*args, **kwargs)
                        span.set_output(result, max_length)
                        return result
                    except Exception as e:
                        span.set_attribute("error.type", type(e).__name__)
                        raise

            return cast(Callable[P, R], sync_wrapper)

        return decorator


def get_tracer(name: str) -> Tracer:
    """Get a Tracer instance by name.

    Args:
        name (str):
            The name of the tracer/service.
    """
    return Tracer(name)


class TracerProvider(_OtelTracerProvider):
    """
    Python wrapper around PyTracerProvider that returns Python Tracer instances.

    This wrapper ensures that get_tracer() returns the Python Tracer class
    with decorator support, not the Rust BaseTracer.
    """

    def __init__(
        self,
        transport_config: Optional[Any] = None,
        exporter: Optional[Any] = None,
        batch_config: Optional[BatchConfig] = None,
        sample_ratio: Optional[float] = None,
        scouter_queue: Optional[Any] = None,
        default_attributes: Optional[Attributes] = None,
        default_entity_uid: Optional[str] = None,
    ):
        """Initialize TracerProvider and underlying Rust tracer."""

        self.transport_config = transport_config
        self.exporter = exporter
        self.batch_config = batch_config
        self.sample_ratio = sample_ratio
        self.scouter_queue = scouter_queue
        self.default_attributes = default_attributes
        self.default_entity_uid = default_entity_uid
        self._tracer_cache: dict[tuple[str, str | None, str | None], _OtelTracer] = {}
        self._tracer_cache_lock = threading.Lock()

    def get_tracer(
        self,
        instrumenting_module_name: str,
        instrumenting_library_version: Optional[str] = None,
        schema_url: Optional[str] = None,
        attributes: Optional[Attributes] = None,
    ) -> _OtelTracer:
        """
        Get a Python Tracer instance with decorator support.

        This method returns the Python Tracer class that wraps BaseTracer,
        providing the @tracer.span() decorator functionality.

        Args:
            instrumenting_module_name (str):
                Module name (typically __name__)
            instrumenting_library_version (Optional[str]):
                Optional version string
            schema_url (Optional[str]):
                Optional schema URL
            attributes (Optional[Attributes]):
                Optional attributes dict

        Returns:
            Tracer: Python Tracer instance with decorator support
        """

        cache_key = (
            instrumenting_module_name,
            instrumenting_library_version,
            schema_url,
        )
        if cache_key in self._tracer_cache:
            return self._tracer_cache[cache_key]

        with self._tracer_cache_lock:
            if cache_key in self._tracer_cache:
                return self._tracer_cache[cache_key]

            tracer = cast(
                _OtelTracer,
                init_tracer(
                    service_name=instrumenting_module_name,
                    scope=instrumenting_library_version,  # type: ignore
                    transport_config=self.transport_config,
                    exporter=self.exporter,
                    batch_config=self.batch_config,
                    sample_ratio=self.sample_ratio,
                    scouter_queue=self.scouter_queue,
                    schema_url=schema_url,
                    scope_attributes=attributes,  # type: ignore
                    default_attributes=self.default_attributes,  # type: ignore
                    default_entity_uid=self.default_entity_uid,
                ),
            )
            self._tracer_cache[cache_key] = tracer
            return tracer

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        """Force flush all pending spans."""
        flush_tracer()
        return True

    def shutdown(self) -> None:
        """Shutdown the tracer provider."""
        shutdown_tracer()


class ScouterInstrumentor(BaseInstrumentor):
    """
    OpenTelemetry-compatible instrumentor for Scouter tracing.

    Provides a standard instrument() interface that integrates with
    the OpenTelemetry SDK while using Scouter's Rust-based tracer.

    Examples:
        Basic usage:
        >>> from scouter.tracing import ScouterInstrumentor
        >>> from scouter import BatchConfig, GrpcConfig
        >>>
        >>> instrumentor = ScouterInstrumentor()
        >>> instrumentor.instrument(
        ...     transport_config=GrpcConfig(),
        ...     batch_config=BatchConfig(scheduled_delay_ms=200),
        ... )

        Auto-instrument on import:
        >>> from scouter.tracing import ScouterInstrumentor
        >>> ScouterInstrumentor().instrument()

        Cleanup:
        >>> instrumentor.uninstrument()
    """

    _instance: Optional["ScouterInstrumentor"] = None
    _provider: Optional[TracerProvider] = None

    def __new__(cls) -> "ScouterInstrumentor":
        if cls._instance is None:
            cls._instance = object.__new__(cls)
        return cls._instance

    def __init__(self) -> None:
        pass

    def instrumentation_dependencies(self) -> Collection[str]:
        """Return list of packages required for instrumentation."""
        return []

    def _instrument(self, **kwargs) -> None:
        """Initialize Scouter tracing and set as global provider."""
        if not HAS_OPENTELEMETRY:
            raise ImportError(
                "OpenTelemetry is required for instrumentation. " "Install with: pip install opsml[opentelemetry]"
            )

        if self._provider is not None:
            import logging

            logging.getLogger("scouter.tracing").warning(
                "ScouterInstrumentor is already instrumented. "
                "ScouterInstrumentor is process-wide — call uninstrument() first to reconfigure. "
                "The existing provider will be used."
            )
            return

        eval_profiles: Optional[List["AgentEvalProfile"]] = kwargs.pop("eval_profiles", None)
        if eval_profiles:
            kwargs["default_entity_uid"] = eval_profiles[0].config.uid

        tracer_provider = kwargs.pop("tracer_provider", None)

        if tracer_provider is not None:
            self._provider = tracer_provider
        else:
            self._provider = TracerProvider(
                transport_config=kwargs.pop("transport_config", None),
                exporter=kwargs.pop("exporter", None),
                batch_config=kwargs.pop("batch_config", None),
                sample_ratio=kwargs.pop("sample_ratio", None),
                scouter_queue=kwargs.pop("scouter_queue", None),
                default_attributes=kwargs.pop("attributes", None),
                default_entity_uid=kwargs.pop("default_entity_uid", None),
            )

        from opentelemetry import trace

        try:
            trace._TRACER_PROVIDER_SET_ONCE._done = False  # pylint: disable=protected-access
            trace._TRACER_PROVIDER_SET_ONCE._lock = __import__("threading").Lock()  # pylint: disable=protected-access
        except AttributeError:
            import logging as _logging

            _logging.getLogger("scouter.tracing").warning(
                "Could not reset OTel provider guard — opentelemetry-api internals may have "
                "changed. Proceeding anyway."
            )
        set_tracer_provider(self._provider)

        propagate_baggage = kwargs.pop("propagate_baggage", True)

        # Register W3C TraceContext + Baggage propagators so that third-party
        # instrumentors (StarletteInstrumentor, HTTPXInstrumentor, etc.) can
        # inject and extract traceparent/tracestate headers transparently.
        try:
            from opentelemetry.propagate import set_global_textmap
            from opentelemetry.propagators.composite import CompositePropagator
            from opentelemetry.trace.propagation.tracecontext import (
                TraceContextTextMapPropagator,
            )

            if propagate_baggage:
                from opentelemetry.baggage.propagation import W3CBaggagePropagator

                set_global_textmap(
                    CompositePropagator(
                        [
                            TraceContextTextMapPropagator(),
                            W3CBaggagePropagator(),
                        ]
                    )
                )
            else:
                set_global_textmap(
                    CompositePropagator(
                        [
                            TraceContextTextMapPropagator(),
                        ]
                    )
                )
        except ImportError:
            pass  # opentelemetry-api not fully installed; propagator setup skipped

    def instrument(
        self,
        transport_config: Optional[Any] = None,
        exporter: Optional[Any] = None,
        batch_config: Optional[BatchConfig] = None,
        sample_ratio: Optional[float] = None,
        scouter_queue: Optional[Any] = None,
        attributes: Optional[Attributes] = None,
        eval_profiles: Optional[List["AgentEvalProfile"]] = None,
        propagate_baggage: bool = True,
        **kwargs,
    ) -> None:
        """
        Instrument with Scouter tracing and set as global OpenTelemetry provider.

        Args:
            transport_config (Optional[Any]):
                Export configuration (OtelExportConfig, etc.)
            exporter (Optional[Any]):
                Custom span exporter instance
            batch_config (Optional[BatchConfig]):
                Batch processing configuration
            sample_ratio (Optional[float]):
                Sampling ratio (0.0 to 1.0)
            scouter_queue (Optional[Any]):
                Optional ScouterQueue for buffering
            attributes (Optional[Attributes]):
                Optional attributes to set on every span created by this tracer
            eval_profiles (Optional[List[AgentEvalProfile]]):
                Optional agent eval profiles. The first profile UID becomes the
                default entity tag materialized on each span as
                `scouter.entity.{uid}={uid}` unless overridden by
                `active_profile(...)`.
            propagate_baggage (bool):
                Whether W3C baggage propagation should be globally enabled.
            **kwargs:
                Additional keyword arguments for TracerProvider initialization

        """
        super().instrument(
            transport_config=transport_config,
            exporter=exporter,
            batch_config=batch_config,
            sample_ratio=sample_ratio,
            scouter_queue=scouter_queue,
            attributes=attributes,
            eval_profiles=eval_profiles,
            propagate_baggage=propagate_baggage,
            **kwargs,
        )

    def enable_local_capture(self) -> None:
        """Enable local span capture mode on the ScouterSpanExporter."""
        get_tracer("scouter").enable_local_capture()

    def disable_local_capture(self) -> None:
        """Disable local span capture mode, discarding any buffered spans."""
        get_tracer("scouter").disable_local_capture()

    def drain_local_spans(self) -> List[TraceSpanRecord]:
        """Drain and return all locally captured spans, clearing the buffer."""
        return get_tracer("scouter").drain_local_spans()

    def get_local_spans_by_trace_ids(self, trace_ids: List[str]) -> List[TraceSpanRecord]:
        """Return captured spans matching the given trace IDs without draining the buffer."""
        return get_tracer("scouter").get_local_spans_by_trace_ids(trace_ids)

    def _uninstrument(self, **kwargs) -> None:
        """Shutdown Scouter tracing and reset global provider."""
        if not HAS_OPENTELEMETRY:
            return

        if self._provider is not None:
            self._provider.shutdown()
            self._provider = None
        else:
            try:
                flush_tracer()
            except Exception:  # noqa: BLE001 pylint: disable=broad-except
                pass
            try:
                shutdown_tracer()
            except Exception:  # noqa: BLE001 pylint: disable=broad-except
                pass

        from opentelemetry import trace

        try:
            trace._TRACER_PROVIDER = None  # pylint: disable=protected-access
            trace._TRACER_PROVIDER_SET_ONCE._done = False  # pylint: disable=protected-access
        except AttributeError:
            pass

        # Reset the singleton
        ScouterInstrumentor._instance = None

        assert self._provider is None, "Expected provider to be None after uninstrument()"

    @property
    def is_instrumented(self) -> bool:
        """Check if instrumentation is active."""
        return self._provider is not None


# Convenience function matching common pattern
def instrument(
    transport_config: Optional[Any] = None,
    exporter: Optional[Any] = None,
    batch_config: Optional[BatchConfig] = None,
    sample_ratio: Optional[float] = None,
    scouter_queue: Optional[Any] = None,
    attributes: Optional[Attributes] = None,
    eval_profiles: Optional[List["AgentEvalProfile"]] = None,
    propagate_baggage: bool = True,
) -> None:
    """
    Convenience function to instrument with Scouter tracing.

    This is equivalent to:
        ScouterInstrumentor().instrument(**kwargs)

    Args:
        transport_config (Optional[Any]):
            Export configuration (OtelExportConfig, etc.)
        exporter (Optional[Any]):
            Custom span exporter instance
        batch_config (Optional[BatchConfig]):
            Batch processing configuration
        sample_ratio (Optional[float]):
            Sampling ratio (0.0 to 1.0)
        scouter_queue (Optional[Any]):
            Optional ScouterQueue for buffering
        attributes (Optional[Attributes]):
            Optional attributes to set on every span created by this tracer
        eval_profiles (Optional[List[AgentEvalProfile]]):
            Optional agent eval profiles. The first profile UID becomes the
            default entity tag materialized on each span as
            `scouter.entity.{uid}={uid}` unless overridden by
            `active_profile(...)`.
        propagate_baggage (bool):
            Whether W3C baggage propagation should be globally enabled.

    Examples:
        >>> from scouter.tracing import instrument
        >>> from scouter import BatchConfig, OtelExportConfig, OtelProtocol
        >>>
        >>> instrument(
        ...     transport_config=OtelExportConfig(
        ...         endpoint="http://localhost:4318/v1/traces",
        ...         protocol=OtelProtocol.HttpProtobuf,
        ...     ),
        ...     batch_config=BatchConfig(scheduled_delay_ms=200),
        ... )
    """
    ScouterInstrumentor().instrument(
        transport_config=transport_config,
        exporter=exporter,
        batch_config=batch_config,
        sample_ratio=sample_ratio,
        scouter_queue=scouter_queue,
        attributes=attributes,
        eval_profiles=eval_profiles,
        propagate_baggage=propagate_baggage,
    )


def uninstrument() -> None:
    """
    Convenience function to uninstrument Scouter tracing.

    This is equivalent to:
        ScouterInstrumentor().uninstrument()
    """
    ScouterInstrumentor().uninstrument()


@contextmanager
def active_profile(profile: "AgentEvalProfile") -> Generator[None, None, None]:
    """Set the active agent eval profile UID in OTel baggage context.

    This context manager attaches the profile UID as OTel baggage under the
    canonical key ``scouter.active.entity_uid``. Rust span creation reads this
    baggage value and materializes the authoritative span attribute
    ``scouter.entity.{profile.config.uid}={profile.config.uid}``.

    If ``opentelemetry`` is not installed, the context manager is a no-op.

    Args:
        profile (AgentEvalProfile):
            The agent eval profile to activate.
    """
    try:
        from opentelemetry import baggage
        from opentelemetry import context as context_api
    except ImportError:
        yield
        return

    ctx = baggage.set_baggage(
        SCOUTER_ACTIVE_ENTITY_UID_BAGGAGE_KEY,
        profile.config.uid,
        context=context_api.get_current(),
    )
    token = context_api.attach(ctx)
    try:
        yield
    finally:
        context_api.detach(token)


__all__ = [
    "Tracer",
    "TracerProvider",
    "get_tracer",
    "init_tracer",
    "SpanKind",
    "FunctionType",
    "ActiveSpan",
    "OtelExportConfig",
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
    "get_tracing_headers_from_current_span",
    "extract_span_context_from_headers",
    "get_current_active_span",
    "ScouterInstrumentor",
    "ScouterTracingMiddleware",
    "instrument",
    "uninstrument",
    "active_profile",
    "enable_local_span_capture",
    "disable_local_span_capture",
    "drain_local_span_capture",
]
