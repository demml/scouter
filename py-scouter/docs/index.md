<h1 align="center">
  <br>
  <img src="https://github.com/demml/scouter/blob/main/images/scouter-logo.png?raw=true"  width="600" alt="scouter logo"/>
  <br>
</h1>

<h2 align="center"><b>Developer-First ML Monitoring, Observability, and GenAI Evaluation</b></h2>

## Table of Contents

- [What is it?](#what-is-it)
- [Why Use It?](#why-use-it)
- [Developer-First Experience](#developer-first-experience)
- [Production Ready](#production-ready)
- [Quick Start](#quick-start)
  - [Traditional Monitoring](#traditional-monitoring)
  - [Distributed Tracing](#distributed-tracing)
  - [GenAI Evaluation](#genai-evaluation)
    - [Offline Evaluation](#offline-evaluation--regression-testing-before-you-ship)
    - [Online Evaluation](#online-evaluation--continuous-production-monitoring)
- [Supported Data Types](#supported-data-types)

---

## **What is it?**

`Scouter` is a developer-first monitoring and observability toolkit for ML and AI workflows. It covers the full spectrum of production AI observability — from traditional data and model drift detection, to distributed tracing, to online and offline GenAI evaluation. Built entirely in `Rust` with `Postgres` as its primary data store, and exposed to Python via PyO3-generated stubs.

## **Why Use It?**

Because you deploy ML and AI services that need to be monitored, and you want a single toolkit that handles drift detection, distributed tracing, and GenAI evaluation — without stitching together five different libraries.


### Developer-First Experience
- **Zero-friction Integration** - Drop into existing ML and AI workflows in minutes
- **Type-safe by Design** - The entire codebase is Rust<sup>*</sup>. Python users interact via PyO3-generated stubs. Catch errors before they hit production
- **One Dependency** - Monitoring, tracing, and GenAI evaluation in a single library. No need to install multiple libraries
- **Standardized Patterns** - Out of the box patterns for drift monitoring, distributed tracing, and LLM evaluation
- **Offline → Online Parity** - Define your GenAI evaluation tasks once; run them as offline regression tests and as live production monitors
- **Integrations** - Works out of the box with any Python API framework. Event-driven transport support for `Kafka`, `RabbitMQ`, and `Redis`

### Production Ready
- **High-Performance Server** - Built entirely in Rust with Axum for speed, reliability, and concurrency
- **Cloud-Ready** - Native support for AWS, GCP, Azure
- **Modular Design** - Use what you need, leave what you don't
- **Alerting and Monitoring** - Built-in alerting integrations with `Slack` and `OpsGenie` to notify you and your team when an alert is triggered
- **Data Retention** - Built-in data retention policies to keep your database clean and performant
- **OpenTelemetry Compatible** - Drop Scouter in as a `TracerProvider`; spans flow to both Scouter's backend and any external OTEL collector

<sup>
* Scouter is written entirely in Rust and exposed via a Python API built with PyO3.
</sup>

---

## Quick Start

Scouter follows a client and server architecture — the client is a lightweight Python library (backed by Rust) that drops into any application, and the server handles data collection, storage, drift computation, tracing, and evaluation.

### Install Scouter
```bash
pip install scouter-ml
```

Set the server URI before using the client:

```bash
export SCOUTER_SERVER_URI=http://your-scouter-server:8000
```

---

## Traditional Monitoring

Out of the box, Scouter provides three drift detection strategies with automated alerting:

- **Population Stability Index (PSI)** — Detects distribution shift by binning baseline data into deciles and computing PSI per feature
- **Statistical Process Control (SPC)** — Grand mean + stddev control limits (1σ/2σ/3σ) with WECO rules for out-of-control detection
- **Custom Metrics** — Any named metric with `Above`, `Below`, or `Outside` alert thresholds

### PSI Quickstart

```python
import uvicorn
from contextlib import asynccontextmanager
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter import (
    CommonCrons,
    Drifter,
    HttpConfig,
    PsiAlertConfig,
    PsiDriftConfig,
    ScouterClient,
    ScouterQueue,
)
from scouter.util import FeatureMixin


class PredictRequest(BaseModel, FeatureMixin):
    feature_1: float
    feature_2: float
    feature_3: float


def create_psi_profile(data):
    drifter = Drifter()
    client = ScouterClient()

    psi_config = PsiDriftConfig(
        space="production",
        name="my_model",
        version="0.0.1",
        alert_config=PsiAlertConfig(
            schedule=CommonCrons.Every6Hours,
            features_to_monitor=["feature_1", "feature_2"],
        ),
    )

    profile = drifter.create_drift_profile(data, psi_config)
    client.register_profile(profile=profile, set_active=True)
    return profile.save_to_json()


if __name__ == "__main__":
    profile_path = create_psi_profile(training_data)

    @asynccontextmanager
    async def lifespan(fast_app: FastAPI):
        fast_app.state.queue = ScouterQueue.from_path(
            path={"my_model": profile_path},
            transport_config=HttpConfig(),
        )
        yield
        fast_app.state.queue.shutdown()

    app = FastAPI(lifespan=lifespan)

    @app.post("/predict")
    async def predict(request: Request, payload: PredictRequest):
        # Non-blocking insert — <1µs latency impact
        request.app.state.queue["my_model"].insert(payload.to_features())
        return {"message": "success"}

    uvicorn.run(app, host="0.0.0.0", port=8888)
```

---

## Distributed Tracing

Scouter implements the OpenTelemetry `BaseInstrumentor` interface. Drop it in as a `TracerProvider` alongside your existing OTEL stack, or use it standalone. Every span is exported to Scouter's backend; an optional second export path sends to any OTEL-compatible collector.

```python
from scouter.tracing import ScouterInstrumentor, get_tracer

# Registers Scouter as the global OTEL TracerProvider
# Any OTEL auto-instrumentation library (FastAPI, httpx, etc.) routes spans through Scouter automatically
ScouterInstrumentor().instrument()

tracer = get_tracer(name="my-service")

@tracer.span("process_request")
async def process_request(payload: dict) -> dict:
    # Inputs, outputs, and exceptions captured automatically
    return {"result": "ok"}
```

Correlated tracing and queue insertion in a single span:

```python
with tracer.start_as_current_span("inference") as span:
    response = call_llm(user_input)
    span.insert_queue_item("genai_profile", GenAIEvalRecord(context={...}))
```

See the [Tracing Overview](docs/tracing/overview.md) for cross-service context propagation, sync/async/streaming support, and OTEL collector configuration.

---

## GenAI Evaluation

Scouter provides three evaluation primitives that work identically in offline batch tests and online production monitors:

- **`AssertionTask`** — Deterministic rule-based checks. 50+ `ComparisonOperator` values covering numeric, string, collection, length, type, and format validation. Zero cost, minimal latency.
- **`LLMJudgeTask`** — LLM-powered semantic evaluation (relevance, quality, hallucination, tone). Structured output via Pydantic. Supports OpenAI, Anthropic, and Google providers.
- **`TraceAssertionTask`** — Validates properties of distributed traces captured by Scouter's tracing system: span execution order, retry counts, token budgets, latency SLAs, error counts, and model attribution. Zero cost; bridges tracing and evaluation in the same task graph.

Tasks support dependency graphs and conditional execution gates, so you can build multi-stage evaluation pipelines and prevent expensive LLM calls when upstream checks fail.

### Offline Evaluation — Regression Testing Before You Ship

Run batch evaluations against a test set. Use task dependencies and `TraceAssertionTask` to validate both what your agent returned and how it executed.

```python
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalDataset,
    LLMJudgeTask,
)
from scouter.genai import Prompt, Provider, Score
from scouter.queue import GenAIEvalRecord

quality_prompt = Prompt(
    messages=(
        "Rate the quality of this response on a scale of 1-5.\n\n"
        "Query: ${query}\nResponse: ${response}"
    ),
    model="gemini-2.5-flash-lite",
    provider=Provider.Gemini,
    output_type=Score,
)

tasks = [
    # Fast gate — skip the LLM call if the response is empty
    AssertionTask(
        id="not_empty",
        context_path="response",
        operator=ComparisonOperator.HasLengthGreaterThan,
        expected_value=10,
        condition=True,
    ),
    # LLM judge — only runs if not_empty passes
    LLMJudgeTask(
        id="quality_check",
        prompt=quality_prompt,
        expected_value=4,
        context_path="score",
        operator=ComparisonOperator.GreaterThanOrEqual,
        depends_on=["not_empty"],
        description="Quality score must be >= 4/5",
    ),
]

records = [
    GenAIEvalRecord(context={"query": q, "response": r})
    for q, r in test_pairs
]

dataset = GenAIEvalDataset(records=records, tasks=tasks)
dataset.print_execution_plan()        # Preview task graph before running
results = dataset.evaluate()
results.as_table()                    # Workflow summary
results.as_table(show_tasks=True)     # Per-task breakdown
```

**Trace Assertion Example — Validate How Your Agent Executed**

`TraceAssertionTask` evaluates span properties from Scouter's tracing system. Enforce execution order, token budgets, and SLAs in the same task graph as your LLM judges — no extra tooling required.

```python
from scouter.evaluate import (
    TraceAssertionTask,
    TraceAssertion,
    AggregationType,
    SpanFilter,
    ComparisonOperator,
)

trace_tasks = [
    # Verify the agent ran steps in the correct order
    TraceAssertionTask(
        id="execution_order",
        assertion=TraceAssertion.span_sequence(["retrieve", "rerank", "generate"]),
        operator=ComparisonOperator.Equals,
        expected_value=True,
        condition=True,  # Gate — skip downstream checks if order is wrong
        description="Verify correct pipeline execution order",
    ),
    # Enforce token budget across all LLM calls
    TraceAssertionTask(
        id="token_budget",
        assertion=TraceAssertion.span_aggregation(
            filter=SpanFilter.by_name_pattern(r"llm\..*"),
            attribute_key="token_count",
            aggregation=AggregationType.Sum,
        ),
        operator=ComparisonOperator.LessThan,
        expected_value=10_000,
        depends_on=["execution_order"],
        description="Total tokens must stay under budget",
    ),
    # Enforce latency SLA
    TraceAssertionTask(
        id="latency_sla",
        assertion=TraceAssertion.trace_duration(),
        operator=ComparisonOperator.LessThan,
        expected_value=5000.0,  # 5 seconds in ms
        depends_on=["execution_order"],
        description="End-to-end trace must complete within 5s",
    ),
]
```

<table>
  <tr>
    <td align="center"><b>Offline Evaluation</b></td>
    <td align="center"><b>Regression Testing (Comparison)</b></td>
  </tr>
  <tr>
    <td><img src="https://github.com/demml/scouter/blob/main/images/offline_evaluation.png?raw=true" alt="Offline Evaluation"/></td>
    <td><img src="https://github.com/demml/scouter/blob/main/images/regression_testing.png?raw=true" alt="Regression Testing"/></td>
  </tr>
</table>

### Online Evaluation — Continuous Production Monitoring

Register the same tasks as a production drift profile. The server samples traffic, runs evaluations asynchronously without blocking your application, and alerts when pass rates drop.

```python
from scouter import (
    AlertCondition,
    AlertThreshold,
    GenAIAlertConfig,
    GenAIEvalConfig,
    ScouterClient,
    SlackDispatchConfig,
)
from scouter.evaluate import GenAIEvalProfile

alert_config = GenAIAlertConfig(
    dispatch_config=SlackDispatchConfig(channel="#ml-alerts"),
    schedule="0 */6 * * *",
    alert_condition=AlertCondition(
        baseline_value=0.80,   # Alert if pass rate drops below 75% (0.80 - 0.05)
        alert_threshold=AlertThreshold.Below,
        delta=0.05,
    ),
)

config = GenAIEvalConfig(
    space="production",
    name="support_agent",
    version="1.0.0",
    sample_ratio=0.10,   # Evaluate 10% of requests
    alert_config=alert_config,
)

# Reuse the same tasks defined for offline regression testing
profile = GenAIEvalProfile(config=config, tasks=tasks)

client = ScouterClient()
client.register_profile(profile, set_active=True)

# At request time — non-blocking, evaluated asynchronously server-side
record = GenAIEvalRecord(context={"query": user_query, "response": model_output})
queue["support_agent"].insert(record)
```

!!!success
    That's it! Define your evaluation tasks once, use them both for pre-deployment regression testing and continuous production monitoring. See the [GenAI Evaluation docs](docs/monitoring/genai/overview.md) for task dependency graphs, conditional gates, and multi-stage evaluation workflows.

---

## Supported Data Types

Scouter accepts **Pandas DataFrames**, **Polars DataFrames**, **NumPy 2D arrays**, and **Pydantic models** out of the box.
