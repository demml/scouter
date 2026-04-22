# Examples

If you are working on agent evaluation, start with `examples/evaluate/simple`.

That folder keeps the contract tight:

- shared `prompt.yaml`
- shared `task.yaml`
- shared `scenarios.jsonl`
- one shared `setup.py`
- one framework-specific eval runner per agent framework

## Recommended paths

| Path | Use it for |
|---|---|
| `examples/evaluate/simple/` | Canonical single-agent offline eval examples for ADK, CrewAI, and OpenAI Agents SDK |
| `examples/evaluate/multi_agent/` | Multi-agent CrewAI reference example |
| `examples/evaluate/react/adk/` | Reactive ADK evaluation pattern |
| `examples/tracing/` | Tracing-only setup and framework instrumentation |
| `examples/monitor/` | Drift monitoring examples |
| `examples/profile/` | Profile creation examples |
