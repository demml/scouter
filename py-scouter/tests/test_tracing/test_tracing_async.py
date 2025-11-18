import asyncio
from typing import AsyncGenerator

import pytest

from .conftest import ChatInput


@pytest.mark.asyncio
async def test_async_decorator_basic(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("async_test_function")
    async def async_test_function(x: int) -> int:
        await asyncio.sleep(0.01)
        return x * 2

    result = await async_test_function(5)

    assert result == 10
    assert len(span_exporter.spans) == 1
    assert span_exporter.spans[0].span_name == "async_test_function"


@pytest.mark.asyncio
async def test_async_decorator_with_pydantic(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("async_process_chat")
    async def async_process_chat(model: ChatInput, delay: float) -> dict:
        await asyncio.sleep(delay)
        return {"message": model.message, "user_id": model.user_id, "processed": True}

    chat_input = ChatInput(message="Async Hello", user_id=2)
    result = await async_process_chat(chat_input, 0.01)

    assert result["message"] == "Async Hello"
    assert result["processed"] is True
    assert len(span_exporter.spans) == 1


@pytest.mark.asyncio
async def test_async_decorator_exception_handling(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("async_failing_function")
    async def async_failing_function():
        await asyncio.sleep(0.01)
        raise RuntimeError("Async test exception")

    with pytest.raises(RuntimeError, match="Async test exception"):
        await async_failing_function()

    assert len(span_exporter.spans) == 1


@pytest.mark.asyncio
async def test_async_generator_decorator(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("async_generate_numbers", capture_last_stream_item=True)
    async def async_generate_numbers(count: int) -> AsyncGenerator[int, None]:
        for i in range(count):
            await asyncio.sleep(0.001)
            yield i * i

    numbers = []
    async for number in async_generate_numbers(5):
        numbers.append(number)

    assert numbers == [0, 1, 4, 9, 16]
    assert len(span_exporter.spans) == 1


@pytest.mark.asyncio
async def test_async_concurrent_operations(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("async_task")
    async def async_task(task_id: int, delay: float) -> dict:
        await asyncio.sleep(delay)
        return {"task_id": task_id, "completed": True}

    tasks = [async_task(1, 0.01), async_task(2, 0.01), async_task(3, 0.01)]
    results = await asyncio.gather(*tasks)

    assert len(results) == 3
    assert all(result["completed"] for result in results)
    assert len(span_exporter.spans) == 3


@pytest.mark.asyncio
async def test_nested_decorated_functions(tracer, span_exporter):
    span_exporter.clear()

    @tracer.span("outer_async_function")
    async def outer_async_function(x: int) -> int:
        return await inner_async_function(x) * 2

    @tracer.span("inner_async_function")
    async def inner_async_function(x: int) -> int:
        await asyncio.sleep(0.01)
        return x + 1

    result = await outer_async_function(5)
    assert result == 12
    assert len(span_exporter.spans) == 2
    assert span_exporter.spans[1].span_id == span_exporter.spans[0].parent_span_id
