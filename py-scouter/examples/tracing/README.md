# Tracing examples

These examples show how to instrument a Python application with Scouter tracing.

## `instrumentor_example.py`

Demonstrates `ScouterInstrumentor` — the recommended entry point for production
applications.

`ScouterInstrumentor` registers Scouter as the **global OpenTelemetry
`TracerProvider`**.  This means any library that already emits OTel spans
(httpx, SQLAlchemy, FastAPI, gRPC, etc.) will automatically route those spans to
Scouter without per-library configuration.

### What the example covers

| Section | Concept |
|---------|---------|
| `instrument()` | Setup at startup — transport, batching, sampling, default attributes |
| `trace.get_tracer()` | Standard OTel API works after instrumentation |
| `get_tracer()` | Scouter tracer with `.span()` decorator |
| Nested spans | Child spans created inside a root span |
| Baggage | Propagating context across service boundaries |
| Default attributes | `env`, `version`, etc. stamped on every span automatically |
| `uninstrument()` | Clean shutdown, flushes export queue |

### Run it

```bash
# This example assumes you have a Scouter server running
cd py-scouter
uv run python examples/tracing/instrumentor_example.py
```

### `ScouterInstrumentor` vs `init_tracer`

Use `ScouterInstrumentor` when you want to capture spans from third-party
libraries automatically.  Use `init_tracer` / `get_tracer` when you only need
to instrument your own code.

| | `ScouterInstrumentor` | `init_tracer` |
|---|---|---|
| Sets global OTel provider | Yes | Yes |
| Third-party auto-instrumentation | Works automatically | Works automatically |
| Decorator support | Via `get_tracer()` | Via `get_tracer()` |
| Singleton guard | Built-in | Manual |
| Recommended for | Application entrypoints | Library / module-level use |
