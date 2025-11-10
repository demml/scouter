# type: ignore
import functools
from typing import (
    Any,
    AsyncGenerator,
    Awaitable,
    Callable,
    Generator,
    List,
    Optional,
    ParamSpec,
    TypeVar,
    cast,
)

from .. import tracing

P = ParamSpec("P")
R = TypeVar("R")


init_tracer = tracing.init_tracer
SpanKind = tracing.SpanKind
FunctionType = tracing.FunctionType
get_tracer = tracing.get_tracer
get_function_type = tracing.get_function_type


# _current_span: ContextVar[Optional[tracing.ActiveSpan]] = ContextVar(
#    "_current_active_span", default=None
# )


# def get_current_span() -> Optional[tracing.ActiveSpan]:
#    """
#    Get the currently active span.
#
#    This is a helper function to retrieve the currently active span when using the
#    tracing decorator.
#
#    Returns:
#        The currently active ActiveSpan, or None if no span is active.
#
#    Example:
#        >>> @tracer.span("my_operation")
#        ... def my_function():
#        ...     span = get_current_span()
#        ...     if span:
#        ...         span.set_attribute("custom_key", "custom_value")
#        ...         span.add_event("custom_event", {"detail": "some detail"})
#    """
#    return _current_span.get()
#

# def set_current_span(span: Optional[tracing.ActiveSpan]) -> None:
#    """Set the current active span (internal use)."""
#    _current_span.set(span)


def set_output(
    span: tracing.ActiveSpan,
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


class Tracer(tracing.BaseTracer):
    """Extended tracer with decorator support for all function types."""

    def span(
        self,
        name: str,
        kind: SpanKind = SpanKind.Internal,
        label: Optional[str] = None,
        attributes: Optional[dict[str, str]] = None,
        baggage: Optional[dict[str, str]] = None,
        tags: Optional[dict[str, str]] = None,
        parent_context_id: Optional[str] = None,
        max_length: int = 1000,
        capture_last_stream_item: bool = False,
        join_stream_items: bool = False,
        *args,
        **kwargs,
    ) -> Callable[[Callable[P, R]], Callable[P, R]]:
        """Decorator to trace function execution with OpenTelemetry spans.

        Args:
            name (str):
                The name of the span
            kind (SpanKind):
                The kind of span (default: Internal)
            label (Optional[str]):
                An optional label for the span
            attributes (Optional[dict[str, str]]):
                Additional attributes to set on the span
            baggage (Optional[dict[str, str]]):
                Baggage items to attach to the span
            tags (Optional[dict[str, str]]):
                Additional tags to set on the span
            parent_context_id (Optional[str]):
                Parent context ID for the span
            max_length (int):
                Maximum length for input/output capture
            func_type (FunctionType):
                The type of function being decorated (sync, async, generator, async_generator)
            capture_last_stream_item (bool):
                Whether to capture only the last item from streaming functions
            join_stream_items (bool):
                Whether to join all stream items into a single string for output
        Returns:
            Callable[[Callable[P, R]], Callable[P, R]]:
        """

        # I'd prefere this entire decorator to be rust, but creating this type of decorator in rust
        # is a little bit of a pain when dealing with async
        def decorator(func: Callable[P, R]) -> Callable[P, R]:
            span_name = name or f"{func.__module__}.{func.__qualname__}"

            function_type = get_function_type(func)
            if function_type == FunctionType.AsyncGenerator:

                @functools.wraps(func)
                async def async_generator_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    async with self._start_decorated_as_current_span(
                        func,
                        span_name,
                        kind=kind,
                        label=label,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        parent_context_id=parent_context_id,
                        max_length=max_length,
                        func_type=function_type,
                        args=args,
                        kwargs=kwargs,
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

            elif function_type == FunctionType.SyncGenerator:

                @functools.wraps(func)
                def generator_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    with self._start_decorated_as_current_span(
                        func,
                        span_name,
                        kind=kind,
                        label=label,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        parent_context_id=parent_context_id,
                        max_length=max_length,
                        func_type=function_type,
                        args=args,
                        kwargs=kwargs,
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

            elif function_type == FunctionType.Async:

                @functools.wraps(func)
                async def async_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    async with self._start_decorated_as_current_span(
                        func,
                        span_name,
                        kind=kind,
                        label=label,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        parent_context_id=parent_context_id,
                        max_length=max_length,
                        func_type=function_type,
                        args=args,
                        kwargs=kwargs,
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

            else:

                @functools.wraps(func)
                def sync_wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
                    with self._start_decorated_as_current_span(
                        func,
                        span_name,
                        kind=kind,
                        label=label,
                        attributes=attributes,
                        baggage=baggage,
                        tags=tags,
                        parent_context_id=parent_context_id,
                        max_length=max_length,
                        func_type=function_type,
                        args=args,
                        kwargs=kwargs,
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


__all__ = [
    "Tracer",
    "init_tracer",
    "get_tracer",
    "SpanKind",
    "FunctionType",
]
