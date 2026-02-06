from typing import Generator, Optional

import pytest

from .conftest import ChatInput


def test_init_tracer(tracer, span_exporter):
    span_exporter.clear()
    with tracer.start_as_current_span(
        name="task_one",
        label="label_value",
        tags=[
            {"tag1": "foo"},
            {"tag2": "bar"},
        ],
    ) as span:
        span.set_attribute("task", "one")

    assert len(span_exporter.spans) == 1
    assert len(span_exporter.baggage) == 2


def test_sync_decorator_basic_functionality(tracer, span_exporter):
    """Test basic sync decorator creates span correctly."""
    span_exporter.clear()

    @tracer.span("test_function")
    def test_function(x: int) -> int:
        return x * 2

    _result = test_function(5)

    assert len(span_exporter.spans) == 1
    assert span_exporter.spans[0].span_name == "test_function"


def test_sync_decorator_with_pydantic(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("process_chat")
    def process_chat(model: ChatInput, multiplier: int) -> dict:
        return {
            "message": model.message,
            "user_id": model.user_id,
            "multiplier": multiplier,
        }

    chat_input = ChatInput(message="Hello", user_id=1)
    result = process_chat(chat_input, 2)

    assert result["message"] == "Hello"
    assert result["user_id"] == 1
    assert len(span_exporter.spans) == 1


def test_sync_decorator_with_optional_params(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("optional_params")
    def function_with_optionals(required: str, optional: Optional[str] = None) -> dict:
        return {"required": required, "optional": optional}

    result1 = function_with_optionals("test", "optional_value")
    result2 = function_with_optionals("test")

    assert result1["optional"] == "optional_value"
    assert result2["optional"] is None
    assert len(span_exporter.spans) == 2


def test_sync_decorator_exception_handling(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("failing_function")
    def failing_function():
        raise ValueError("Test exception")

    with pytest.raises(ValueError, match="Test exception"):
        failing_function()

    assert len(span_exporter.spans) == 1


def test_sync_generator_decorator(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("generate_numbers", capture_last_stream_item=True)
    def generate_numbers(count: int) -> Generator[int, None, None]:
        for i in range(count):
            yield i * i

    numbers = list(generate_numbers(5))

    assert numbers == [0, 1, 4, 9, 16]
    assert len(span_exporter.spans) == 1


def test_sync_decorator_preserves_metadata(tracer, span_exporter):
    @tracer.span("test_metadata")
    def documented_function(x: int) -> int:
        """This function doubles the input value."""
        return x * 2

    assert documented_function.__doc__ == "This function doubles the input value."
    assert documented_function.__name__ == "documented_function"


def test_nested_decorated_functions(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("outer_function")
    def outer_function(value: int) -> int:
        return inner_function(value) * 2

    @tracer.span("inner_function")
    def inner_function(value: int) -> int:
        return value + 1

    result = outer_function(5)

    assert result == 12
    assert len(span_exporter.spans) == 2
    assert (
        span_exporter.spans[1].span_id == span_exporter.spans[0].parent_span_id
    )  # Nested spans are exported in reverse order
    assert span_exporter.spans[1].parent_span_id is None
    assert span_exporter.spans[0].span_name == "inner_function"
    assert span_exporter.spans[1].span_name == "outer_function"
    a
