# ScouterInstrumentor

`ScouterInstrumentor` implements the standard OpenTelemetry `BaseInstrumentor` interface. Once instrumented, it registers Scouter's `TracerProvider` as the global OTEL provider — any library that calls `opentelemetry.trace.get_tracer()` routes spans through Scouter automatically.

See the [overview comparison table](/scouter/docs/tracing/overview/#scouterinstrumentor-vs-init_tracer) for when to use `ScouterInstrumentor` vs `init_tracer()`.

## Installation

`opentelemetry-sdk` is required. If the package is missing, `instrument()` raises:

```
ImportError: OpenTelemetry is required for instrumentation.
Install with: pip install opsml[opentelemetry]
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

### LlamaIndex

LlamaIndex has built-in OTEL tracing support. Set up Scouter first, then configure LlamaIndex to use the global provider:

```bash
pip install llama-index opentelemetry-sdk
```

```python
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig
from llama_index.core import Settings, VectorStoreIndex, SimpleDirectoryReader
from opentelemetry import trace

# Register Scouter as the global OTEL provider
ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "llamaindex-service"},
)

# LlamaIndex picks up the global provider automatically
documents = SimpleDirectoryReader("./docs").load_data()
index = VectorStoreIndex.from_documents(documents)
query_engine = index.as_query_engine()

# Spans from the query are exported to Scouter
response = query_engine.query("What is the main topic of these documents?")
```

### CrewAI

Install `opentelemetry-instrumentation-crewai` (if available for your version) and register Scouter first:

```bash
pip install crewai opentelemetry-sdk
```

```python
from scouter.tracing import ScouterInstrumentor
from scouter import GrpcConfig
from crewai import Agent, Task, Crew

ScouterInstrumentor().instrument(
    transport_config=GrpcConfig(),
    attributes={"service.name": "crewai-service"},
)

researcher = Agent(
    role="Researcher",
    goal="Find relevant information on the topic",
    backstory="Expert at searching and synthesizing information",
    tools=[...],
)

task = Task(
    description="Research the latest trends in vector databases",
    agent=researcher,
)

crew = Crew(agents=[researcher], tasks=[task])
result = crew.kickoff()
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
