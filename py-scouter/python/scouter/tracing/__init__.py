# type: ignore

import asyncio
import functools
from typing import Any, Callable, Optional, TypeVar, ParamSpec, cast, Awaitable
from contextvars import ContextVar
from .. import tracing
import inspect
from typing import Dict

P = ParamSpec("P")
R = TypeVar("R")


_current_span: ContextVar[Optional[tracing.ActiveSpan]] = ContextVar(
    "_current_active_span", default=None
)


def _capture_function_inputs(
    span: tracing.ActiveSpan,
    func: Callable,
    args: tuple,
    kwargs: dict,
    max_length: int = 1000,
) -> None:
    """
    Capture function inputs as a simple dictionary mapping parameter names to values.
    """
    try:
        sig = inspect.signature(func)
        bound_args = sig.bind(*args, **kwargs)
        bound_args.apply_defaults()
        span.set_input(bound_args.arguments, max_length=max_length)

    except Exception as e:
        span.add_event(
            "scouter.error.input_capture_error",
            {"error": str(e)},
        )


def _capture_function_outputs(
    span: tracing.ActiveSpan,
    output: Any,
    max_length: int = 1000,
) -> None:
    """
    Capture function outputs as a simple dictionary mapping parameter names to values.
    """
    try:
        span.set_output(output, max_length=max_length)
    except Exception as e:
        span.add_event(
            "scouter.error.output_capture_error",
            {"error": str(e)},
        )


def get_current_span() -> Optional[tracing.ActiveSpan]:
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
    return _current_span.get()


def set_current_span(span: Optional[tracing.ActiveSpan]) -> None:
    """Set the current active span (internal use)."""
    _current_span.set(span)


class Tracer(tracing.BaseTracer):
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
        max_length: int = 1000,
    ) -> Callable[[Callable[P, R]], Callable[P, R]]:
        """Decorator to trace function execution with OpenTelemetry spans.

        Args:
            name (Optional[str]):
                The name of the span. If None, defaults to the function's
                module and qualified name.
            kind (Optional[str]):
                The kind of span (e.g., "SERVER", "CLIENT").
            attributes (Optional[dict[str, str]]):
                Additional attributes to set on the span.
            baggage (Optional[dict[str, str]]):
                Baggage items to attach to the span.
            max_length (int):
                Maximum length for serialized function inputs.
        """

        def decorator(func: Callable[P, R]) -> Callable[P, R]:
            span_name = name
            if span_name is None:
                span_name = f"{func.__module__}.{func.__qualname__}"

            is_async = asyncio.iscoroutinefunction(func)

            if is_async:

                @functools.wraps(func)
                async def async_wrapper(*args: P.args, **kwargs: P.kwargs) -> Any:
                    async with self.start_as_current_span(
                        span_name,
                        kind=kind,
                        attributes=attributes,
                        baggage=baggage,
                    ) as span:
                        set_current_span(span)

                        try:
                            span.set_attribute("function.name", func.__name__)
                            span.set_attribute("function.module", func.__module__)
                            span.set_attribute("function.qualname", func.__qualname__)

                            _capture_function_inputs(
                                span,
                                func,
                                args,
                                kwargs,
                                max_length,
                            )

                            async_func = cast(Callable[P, Awaitable[Any]], func)
                            result = await async_func(*args, **kwargs)

                            _capture_function_outputs(
                                span,
                                result,
                                max_length,
                            )

                            return result
                        except Exception as e:
                            span.set_attribute("error.type", type(e).__name__)
                            raise
                        finally:
                            set_current_span(None)

                return cast(Callable[P, R], async_wrapper)

            else:

                @functools.wraps(func)
                def sync_wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
                    with self.start_as_current_span(
                        span_name,
                        kind=kind,
                        attributes=attributes,
                        baggage=baggage,
                    ) as span:
                        set_current_span(span)

                        try:
                            span.set_attribute("function.name", func.__name__)
                            span.set_attribute("function.module", func.__module__)
                            span.set_attribute("function.qualname", func.__qualname__)

                            _capture_function_inputs(
                                span,
                                func,
                                args,
                                kwargs,
                                max_length,
                            )

                            result = func(*args, **kwargs)

                            _capture_function_outputs(
                                span,
                                result,
                                max_length,
                            )
                            return result
                        except Exception as e:
                            span.set_attribute("error.type", type(e).__name__)
                            raise
                        finally:
                            set_current_span(None)

                return cast(Callable[P, R], sync_wrapper)

        return decorator


def get_tracer(name: str) -> Tracer:
    """Get a Tracer instance by name.

    Args:
        name (str):
            The name of the tracer/service.
    """
    return Tracer(name)


init_tracer = tracing.init_tracer

__all__ = [
    "Tracer",
    "init_tracer",
    "get_tracer",
    "get_current_span",
]
