# ScouterInstrumentor

`ScouterInstrumentor` implements the standard OpenTelemetry `BaseInstrumentor` interface. Once instrumented, it registers Scouter's `TracerProvider` as the global OTEL provider — any library that calls `opentelemetry.trace.get_tracer()` routes spans through Scouter automatically.

See the [overview comparison table](/scouter/docs/tracing/overview/#scouterinstrumentor-vs-init_tracer) for when to use `ScouterInstrumentor` vs `init_tracer()`.

## Installation

`opentelemetry-sdk` is required. If the package is missing, `instrument()` raises:

```
ImportError: OpenTelemetry is required for instrumentation.
Install with: pip install scouter[opentelemetry] if using scouter, or opsml[opentelemetry] if using opsml.
```

Install the dependency:

```bash
pip install opentelemetry-sdk
```

Framework-specific instrumentation libraries are installed separately — see the [Framework Integrations](#framework-integrations) section.

## Basic Usage

```python
from scouter.tracing import ScouterInstrumentor

instrumentor = ScouterInstrumentor()
instrumentor.instrument()
```

Or use the convenience function:

```python
from scouter.tracing import instrument

instrument()
```

`ScouterInstrumentor` is a singleton. Multiple calls to `ScouterInstrumentor()` return the same instance. Calling `instrument()` when already instrumented is a no-op.

## API Reference

### `instrument()`

```python
instrumentor.instrument(
    transport_config: Optional[Any] = None,
    exporter: Optional[Any] = None,
    batch_config: Optional[BatchConfig] = None,
    sample_ratio: Optional[float] = None,
    scouter_queue: Optional[Any] = None,
    attributes: Optional[Attributes] = None,
)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `transport_config` | `HttpConfig`, `GrpcConfig`, `KafkaConfig`, etc. | Export transport to the Scouter backend. |
| `exporter` | `HttpSpanExporter`, `GrpcSpanExporter`, etc. | Optional exporter for an external OTEL collector. |
| `batch_config` | `BatchConfig` | Controls batch export timing and queue size. |
| `sample_ratio` | `float` | Global sampling ratio (0.0–1.0) applied to both Scouter and OTEL exporters. |
| `scouter_queue` | `ScouterQueue` | Optional queue for buffering records alongside spans. |
| `attributes` | `dict[str, Any]` | Key-value attributes stamped on every span created by this tracer. |

### `uninstrument()`

```python
instrumentor.uninstrument()
```

Flushes pending spans, shuts down the `TracerProvider`, and resets the global OTEL provider. The singleton is also reset, so the next call to `ScouterInstrumentor()` creates a fresh instance.

### `is_instrumented`

```python
instrumentor.is_instrumented  # True or False
```

Returns `True` if `instrument()` has been called and the provider is active.

### Local Capture Methods

Useful for testing — captures spans in memory instead of exporting them.

| Method | Description |
|--------|-------------|
| `enable_local_capture()` | Buffer spans locally instead of exporting. |
| `disable_local_capture()` | Stop local capture and discard buffered spans. |
| `drain_local_spans()` | Return and clear all buffered spans as `List[TraceSpanRecord]`. |
| `get_local_spans_by_trace_ids(trace_ids)` | Return buffered spans matching the given trace IDs without clearing the buffer. |

### Convenience Functions

Module-level wrappers around `ScouterInstrumentor()`:

```python
from scouter.tracing import instrument, uninstrument

instrument(transport_config=GrpcConfig(), batch_config=BatchConfig())
# ... later ...
uninstrument()
```

## Default Attributes

The `attributes` parameter stamps a set of key-value pairs on every span created by the tracer. Use this to tag spans with environment or deployment metadata without modifying application code.

```python
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig

ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={
        "deployment.environment": "production",
        "service.version": "2.4.1",
        "team": "ml-platform",
    },
)
```

Every span produced by any OTEL-instrumented library will carry these attributes.

## Framework Integrations

### OpenAI Agents SDK

The `openai-agents` package uses OpenTelemetry natively. After calling `instrument()`, all agent spans are automatically routed through Scouter.

```bash
pip install openai-agents opentelemetry-sdk
```

```python
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig, BatchConfig
import openai
from agents import Agent, Runner

# Instrument before creating agents
ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    batch_config=BatchConfig(scheduled_delay_ms=500),
    attributes={"service.name": "openai-agent-service"},
)

agent = Agent(
    name="ResearchAgent",
    instructions="Search and summarize the given topic.",
    tools=[...],
)

result = Runner.run_sync(agent, "What are the latest advances in transformer architectures?")
# Spans from the agent run are exported to Scouter
```

### Google ADK

Google ADK uses OpenTelemetry natively — it calls `opentelemetry.trace.get_tracer_provider().get_tracer(name)` internally. Calling `instrument()` before any ADK code is all that's needed; there's no separate instrumentation library to register.

```bash
pip install google-adk opentelemetry-sdk
```

```python
# main.py
import asyncio

from google.adk.agents import Agent
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig

# Sets Scouter as the global OTel TracerProvider before any ADK code runs.
# ADK calls get_tracer_provider() internally — it picks this up automatically.
ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "adk-hello-agent"},
)


def get_current_time(city: str) -> dict:
    """Returns the current time in a specified city."""
    return {"city": city, "time": "10:30 AM"}


root_agent = Agent(
    model="gemini-2.0-flash",
    name="hello_agent",
    description="Tells the current time in a specified city.",
    instruction="You are a helpful assistant. Use get_current_time to answer questions about the current time.",
    tools=[get_current_time],
)


async def main() -> None:
    session_service = InMemorySessionService()
    runner = Runner(
        agent=root_agent,
        app_name="hello_app",
        session_service=session_service,
    )

    session = await session_service.create_session(
        app_name="hello_app",
        user_id="user_1",
    )

    message = types.Content(
        role="user",
        parts=[types.Part(text="What time is it in New York?")],
    )

    async for event in runner.run_async(
        user_id="user_1",
        session_id=session.id,
        new_message=message,
    ):
        if event.is_final_response():
            print(event.content.parts[0].text)

    ScouterInstrumentor().uninstrument()


if __name__ == "__main__":
    asyncio.run(main())
```

`run_async` is an async generator that yields events for each step (tool calls, intermediate responses, the final answer). Filtering on `is_final_response()` gives you the displayable text. All spans — agent turns, tool invocations, LLM calls — are exported to Scouter automatically.

### LangChain

Install `opentelemetry-instrumentation-langchain` then call `instrument()` before constructing chains:

```bash
pip install opentelemetry-instrumentation-langchain opentelemetry-sdk
```

```python
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig
from opentelemetry.instrumentation.langchain import LangchainInstrumentor
from langchain_openai import ChatOpenAI
from langchain.chains import LLMChain
from langchain.prompts import PromptTemplate

# Register Scouter as the global OTEL provider first
ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "langchain-service"},
)

# Then register LangChain instrumentation
LangchainInstrumentor().instrument()

llm = ChatOpenAI(model="gpt-4o-mini")
prompt = PromptTemplate.from_template("Summarize the following: {text}")
chain = LLMChain(llm=llm, prompt=prompt)

# LangChain spans flow through Scouter
result = chain.run(text="OpenTelemetry is a set of APIs and SDKs...")
```

### CrewAI

CrewAI uses OpenTelemetry natively. Register Scouter first and all agent, task, and tool spans flow through automatically.

```bash
pip install crewai opentelemetry-sdk
```

```python
from crewai import Agent, Crew, Task
from scouter.tracing import ScouterInstrumentor
from scouter.transport import GrpcConfig

ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "crewai-service"},
)

# Agents and tasks defined after instrument() — spans route through Scouter
researcher = Agent(
    role="Researcher",
    goal="Summarize what OpenTelemetry is in one paragraph",
    backstory="Expert at reading technical documentation",
    allow_delegation=False,
    verbose=True,
)

task = Task(
    description="Write a one-paragraph summary of what OpenTelemetry is.",
    expected_output="A concise paragraph suitable for a developer README.",
    agent=researcher,
)

crew = Crew(agents=[researcher], tasks=[task], verbose=True)
result = crew.kickoff()
print(result)
```

### FastAPI

Use `opentelemetry-instrumentation-fastapi` alongside Scouter. Register Scouter first so FastAPI spans route through the Scouter provider:

```bash
pip install opentelemetry-instrumentation-fastapi opentelemetry-sdk
```

```python
from contextlib import asynccontextmanager
from fastapi import FastAPI
from opentelemetry.instrumentation.fastapi import FastAPIInstrumentor
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig, BatchConfig


@asynccontextmanager
async def lifespan(app: FastAPI):
    # Instrument Scouter as the global OTEL provider
    ScouterInstrumentor().instrument(
        transport_config=GrpcConfig(),
        batch_config=BatchConfig(scheduled_delay_ms=200),
        attributes={
            "service.name": "my-api",
            "deployment.environment": "production",
        },
    )
    # Instrument FastAPI — spans from request handling go through Scouter
    FastAPIInstrumentor.instrument_app(app)

    yield

    ScouterInstrumentor().uninstrument()


app = FastAPI(lifespan=lifespan)


@app.get("/predict")
async def predict(input: str):
    return {"result": run_model(input)}
```

HTTP request spans from FastAPI will appear in Scouter alongside any manual spans you create inside route handlers.

## ScouterTracingMiddleware

`ScouterTracingMiddleware` is a Starlette-compatible middleware that extracts W3C traceparent context from inbound HTTP requests and opens a `SERVER` span as the root for each request. Route handlers get that span as their active context, so any spans created inside are correctly nested as children — no per-endpoint header parsing required.

The middleware works with any Starlette-based framework (FastAPI, Starlette, etc.).

### Setup

```python
from fastapi import FastAPI
from scouter.tracing import ScouterTracingMiddleware

app = FastAPI(lifespan=lifespan)
app.add_middleware(ScouterTracingMiddleware, tracer=tracer)
```

`tracer` must be a Scouter tracer (the return value of `otel_trace.get_tracer_provider().get_tracer(name)` after `instrument()` has been called, or `get_tracer(name)` directly).

### Deferred initialization

FastAPI evaluates `add_middleware()` at import time, before the `lifespan` event runs. If your tracer is set during `lifespan`, you need a proxy:

```python
from typing import Any, Optional
from fastapi import FastAPI
from scouter.tracing import ScouterTracingMiddleware, ScouterInstrumentor, TracerProvider
from scouter.transport import GrpcConfig
from opentelemetry import trace as otel_trace
from contextlib import asynccontextmanager


class _TracerProxy:
    """Forwards attribute access to the real tracer once lifespan sets _inner."""

    def __init__(self) -> None:
        self._inner: Optional[Any] = None

    def __getattr__(self, name: str) -> Any:
        if self._inner is None:
            raise RuntimeError("Tracer not initialized — instrument() must run first")
        return getattr(self._inner, name)


_instrumentor = ScouterInstrumentor()
_tracer = _TracerProxy()


@asynccontextmanager
async def lifespan(app: FastAPI):
    _instrumentor.instrument(
        transport_config=GrpcConfig(),
        attributes={"service.name": "my-service"},
    )
    provider = otel_trace.get_tracer_provider()
    _tracer._inner = provider.get_tracer("my-service")

    yield

    _instrumentor.uninstrument()
    _tracer._inner = None


app = FastAPI(lifespan=lifespan)

# add_middleware runs at import time — _tracer is a proxy here,
# not yet initialized. That's fine: it only resolves _inner at request time.
app.add_middleware(ScouterTracingMiddleware, tracer=_tracer)
```

This is the same pattern used in the [distributed tracing examples](https://github.com/demml/scouter/tree/main/py-scouter/examples/tracing).

### What the middleware produces

For every inbound request, `ScouterTracingMiddleware`:

1. Reads the W3C `traceparent` header (if present)
2. Opens a `SERVER` span with `parent_span_id` set to the upstream caller's span
3. Makes that span the active context for the request lifecycle
4. Records the HTTP method and path on the span
5. Sets span status to `ERROR` if the response status is `>= 500`
6. Closes and exports the span when the response is sent

If no `traceparent` header is present, the middleware opens a root `SERVER` span instead — so uninstrumented callers still get traced.

## Local Span Capture (Testing)

Use local capture to inspect spans produced by your code during tests, without needing a running Scouter backend.

```python
import pytest
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig


@pytest.fixture(autouse=True)
def scouter_local():
    """Set up ScouterInstrumentor with local span capture for tests."""
    inst = ScouterInstrumentor()
    inst.instrument(transport_config=GrpcConfig())
    inst.enable_local_capture()
    yield inst
    inst.disable_local_capture()
    inst.uninstrument()


def test_agent_produces_search_span(scouter_local):
    run_my_agent("What is the weather in NYC?")

    spans = scouter_local.drain_local_spans()
    span_names = [s.name for s in spans]
    assert "web_search" in span_names


def test_filter_by_trace_id(scouter_local):
    trace_id = run_my_agent_and_return_trace_id("query")

    matched = scouter_local.get_local_spans_by_trace_ids([trace_id])
    assert len(matched) > 0
```

`drain_local_spans()` clears the buffer. `get_local_spans_by_trace_ids()` reads without clearing — useful when multiple tests share a buffer or when you want to inspect a specific trace within a larger batch.
