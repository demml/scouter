# Offline Evaluation Examples

These examples show how to use `EvalOrchestrator` to evaluate an AI agent offline against a fixed set of scenarios before deploying to production.

## How it works

1. Define a `GenAIEvalProfile` — the assertions that describe what "good" looks like for your agent.
2. Wrap your agent in an `agent_fn(query: str) -> str` (or subclass `EvalOrchestrator` for async agents).
3. Inside the agent, emit `EvalRecord` objects via `span.add_queue_item(alias, EvalRecord(...))`.
4. Pass your existing `ScouterQueue` to `EvalOrchestrator`. It switches the queue to local capture mode automatically — no records are sent to the server during the run.
5. Call `.run()` to execute all scenarios and get back `ScenarioEvalResults`.

## Examples

| File | What it demonstrates |
|------|---------------------|
| [`simple_eval.py`](simple_eval.py) | Minimal setup: single-turn scenarios, profile assertions, lifecycle |
| [`multi_turn_eval.py`](multi_turn_eval.py) | Multi-turn dialogue, lifecycle hooks (`on_scenario_start` etc.) |
| [`adk_agent_eval.py`](adk_agent_eval.py) | Async Google ADK agent, trace assertions, tool-call verification |
| [`comparison/eval_comparison.py`](comparison/eval_comparison.py) | Baseline vs improved: save to JSON, load, `compare_to()` |

## Evaluation levels

`ScenarioEvalResults` surfaces three levels of metrics:

```
overall_pass_rate
├── dataset_pass_rates["alias"]   ← tasks in GenAIEvalProfile (per EvalRecord)
└── scenario_pass_rate            ← tasks in EvalScenario (per scenario response)
```

## Task types

| Type | Use for |
|------|---------|
| `AssertionTask` | Deterministic checks (field value, string contains, numeric threshold) |
| `TraceAssertionTask` | Span-level assertions (span exists, span count, attribute value) |
| `AgentAssertionTask` | Tool-call assertions on LLM responses (tool called, args, sequence) |
| `LLMJudgeTask` | Semantic evaluation via a judge LLM (see docs for setup) |

## Comparing runs

Save any `ScenarioEvalResults` to disk, load it later, and call `compare_to()`:

```python
baseline = ScenarioEvalResults.load("baseline.json")
current  = ScenarioEvalResults.load("current.json")
comp = current.compare_to(baseline)
comp.as_table()
print("regressed:", comp.regressed)
```
