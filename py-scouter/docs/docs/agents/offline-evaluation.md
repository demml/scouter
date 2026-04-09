# Offline evaluation

Offline evaluation runs your agent against a fixed set of test scenarios and measures quality before anything reaches production. Use it to catch regressions between model versions, validate prompt changes, and build a quality baseline to compare future runs against.

For pre-generated records without a live agent, see [EvalDataset](./eval-dataset.md).

---

## Quick start

```python
from scouter.drift import AgentEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
)
from scouter.transport import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import init_tracer

# 1. Define what to evaluate about your agent's outputs
profile = AgentEvalProfile(
    alias="my_agent",  # matches the alias in span.add_queue_item()
    tasks=[
        AssertionTask(
            id="quality_check",
            context_path="response.quality",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
        ),
    ],
)

# 2. Create a queue from your profile
queue = ScouterQueue.from_profile(
    profile=[profile],
    transport_config=MockConfig(),
)

# 3. Initialize a tracer — your agent emits EvalRecords inside traced spans
tracer = init_tracer(service_name="my-eval", scouter_queue=queue)

# 4. Your agent — emits an EvalRecord, returns a response string
def my_agent(query: str) -> str:
    with tracer.start_as_current_span("agent_call") as span:
        result = {"quality": 9, "text": "Paris is the capital of France."}
        span.add_queue_item("my_agent", EvalRecord(context={"response": result}))
    return result["text"]

# 5. Define test scenarios
scenarios = EvalScenarios(scenarios=[
    EvalScenario(
        id="capital_question",
        initial_query="What is the capital of France?",
    ),
])

# 6. Run
results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent).run()
results.as_table()
```

---

## How it works

### Execution lifecycle

`EvalOrchestrator.run()` manages the full lifecycle:

```
EvalOrchestrator.run()
│
├── enable_capture on queue + span capture buffer
│
│   for each scenario:
│   ├── on_scenario_start(scenario)
│   ├── execute_agent(scenario)          ← calls agent_fn(initial_query)
│   │     └── agent emits EvalRecords via span.add_queue_item()
│   ├── queue.drain_all_records()        ← collect records for this scenario
│   ├── EvalRunner.collect_scenario_data()
│   ├── on_scenario_complete(scenario, response)
│
├── flush_tracer()                       ← ensure spans are in the capture buffer
├── EvalRunner.evaluate()                ← 3-level Rust evaluation engine
├── on_evaluation_complete(results)
│
└── disable_capture [always, even on exception]
```

`EvalRunner` is the Rust evaluation engine. `EvalOrchestrator` is the Python lifecycle wrapper around it.

### 3-level evaluation

`EvalRunner.evaluate()` runs three levels in sequence:

**Level 1: Sub-agent evaluation (workflow)**

For each alias, all records collected across all scenarios are flattened into a single `EvalDataset` and evaluated together. This gives you a holistic quality signal per sub-agent, independent of which scenario produced each record.

```
alias "retriever" → 5 records (one per scenario) → EvalDataset → EvalResults
alias "synthesizer" → 5 records → EvalDataset → EvalResults
```

**Level 2: Scenario-level evaluation**

For each scenario that has `tasks`, a single `EvalRecord` is built from the scenario context (agent response + `expected_outcome`) and evaluated against those tasks. `TraceAssertionTask`s are resolved by matching `trace_id`s from the scenario's records to spans in the capture buffer.

```
scenario "capital_question" → build record from {response, expected_outcome}
  → evaluate scenario tasks → ScenarioResult { passed, pass_rate }
```

**Level 3: Aggregate metrics**

```
EvalMetrics:
  overall_pass_rate     # mean across all dataset + scenario pass rates
  workflow_pass_rate    # mean across sub-agent profile pass rates
  dataset_pass_rates    # per-alias pass rate, e.g. {"retriever": 0.9}
  scenario_pass_rate    # fraction of scenarios where all tasks passed
  total_scenarios       # count
  passed_scenarios      # count
```

### How `trace_id` correlation works

When your agent calls `span.add_queue_item(alias, record)` inside a traced span, the `trace_id` from the active OTel span is automatically stamped onto the `EvalRecord`. This is what connects records to spans for `TraceAssertionTask`.

```
span "agent_call"  →  trace_id = "abc123"
  span.add_queue_item("retriever", record)
    └── record.trace_id is auto-set to "abc123"

During evaluate():
  scenario "q1" has records with trace_id = "abc123"
  TraceAssertionTask filters captured spans by those trace_ids
  → evaluates span assertions against the matching spans
```

---

## Core concepts

### `EvalScenario`

A single test case. At minimum, supply `initial_query`.

```python
EvalScenario(
    id="scenario_id",                         # stable ID for regression tracking
    initial_query="Summarize this article.",
    expected_outcome="A 2-sentence summary.", # available as ${expected_outcome} in tasks
    tasks=[                                   # evaluated against the agent's final response
        AssertionTask(
            id="response_not_empty",
            context_path="response",
            operator=ComparisonOperator.IsString,
            expected_value=True,
        ),
    ],
)
```

Scenario `tasks` evaluate the agent's **final response string**. They're separate from profile tasks, which evaluate each sub-agent's `EvalRecord` context.

### `AgentEvalProfile`

Defines evaluation tasks for one sub-agent. The `alias` must match what you pass to `span.add_queue_item(alias, ...)`.

```python
AgentEvalProfile(
    alias="retriever",
    tasks=[
        AssertionTask(
            id="has_results",
            context_path="results.count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
)
```

### `EvalRecord`

The data your sub-agent emits during execution. Contains a `context` dict that tasks read via `context_path`.

```python
# Inside a traced span:
span.add_queue_item(
    "retriever",
    EvalRecord(context={"results": {"count": 5, "source": "arxiv"}}),
)
# trace_id is stamped automatically from the active span
```

---

## Multi-agent setup

One `AgentEvalProfile` per sub-agent. Register all profiles on the queue.

```python
from scouter.drift import AgentEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
    SpanFilter,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.transport import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor, init_tracer

retriever_profile = AgentEvalProfile(
    alias="retriever",
    tasks=[
        AssertionTask(
            id="has_results",
            context_path="results.count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
        TraceAssertionTask(
            id="retriever_span_emitted",
            assertion=TraceAssertion.span_count(SpanFilter.by_name("retriever_call")),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
)

synthesizer_profile = AgentEvalProfile(
    alias="synthesizer",
    tasks=[
        AssertionTask(
            id="quality_score",
            context_path="response.quality",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
        ),
    ],
)

queue = ScouterQueue.from_profile(
    profile=[retriever_profile, synthesizer_profile],
    transport_config=MockConfig(),
)

# ScouterInstrumentor is required when your profiles include TraceAssertionTask.
# For AssertionTask and LLMJudgeTask only, init_tracer alone is sufficient.
instrumentor = ScouterInstrumentor().instrument(scouter_queue=queue)

tracer = init_tracer(service_name="my-agent", scouter_queue=queue)


def retriever_callback(query: str) -> dict:
    with tracer.start_as_current_span("retriever_call") as span:
        results = {"count": 5, "source": "internal_db"}
        span.add_queue_item("retriever", EvalRecord(context={"results": results}))
    return results


def synthesizer_callback(query: str, context: dict) -> dict:
    with tracer.start_as_current_span("synthesizer_call") as span:
        output = {"quality": 9, "text": f"Answer for: {query}"}
        span.add_queue_item("synthesizer", EvalRecord(context={"response": output}))
    return output


def agent_fn(query: str) -> str:
    with tracer.start_as_current_span("orchestrator"):
        retrieval = retriever_callback(query)
        synthesis = synthesizer_callback(query, retrieval)
    return synthesis["text"]


scenarios = EvalScenarios(scenarios=[
    EvalScenario(id="rag_basics", initial_query="What is RAG?"),
    EvalScenario(id="attention", initial_query="How does attention work?"),
])

results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent_fn).run()
results.as_table()

instrumentor.uninstrument()
```

---

## Multi-turn scenarios

Set `predefined_turns` with follow-up queries. The orchestrator calls `agent_fn` once for `initial_query`, then once per turn in order. The **last response** is used for scenario-level task evaluation.

```python
EvalScenario(
    id="dinner_planning",
    initial_query="Plan a dinner for 4 people.",
    predefined_turns=[
        "Make it vegetarian.",
        "Add a dessert.",
    ],
)
```

`agent_fn` receives each query in isolation. No conversation history is managed automatically. For stateful agents, subclass `EvalOrchestrator` and override `execute_agent`.

---

## Loading scenarios from a file

If your test scenarios live in a file (checked into source control, generated by a pipeline, or maintained by a separate team), use `EvalScenarios.from_path()` instead of building the list in Python:

```python
scenarios = EvalScenarios.from_path("scenarios/qa_suite.jsonl")

results = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent).run()
results.as_table()
```

Supported formats: `.jsonl`, `.json`, `.yaml`, `.yml`. The method raises on anything else.

### JSONL

One scenario per line. Use this for large test suites or when your CI pipeline generates scenarios dynamically — it's streamable and easy to append to without loading the whole file.

```jsonl
{"id": "capital_france", "initial_query": "What is the capital of France?", "expected_outcome": "Paris", "tasks": [{"task_type": "Assertion", "id": "mentions_paris", "context_path": "response", "operator": "Contains", "expected_value": "Paris"}]}
{"id": "water_formula", "initial_query": "What is the chemical formula for water?", "expected_outcome": "H2O", "tasks": [{"task_type": "Assertion", "id": "mentions_h2o", "context_path": "response", "operator": "Contains", "expected_value": "H2O"}]}
```

Tasks in files use a flat array with a `task_type` discriminator — `"Assertion"`, `"LLMJudge"`, or `"TraceAssertion"`. The fields match what you'd pass to the Python constructors.

Parse errors include a line number: `"parse error on line 3: ..."`. Empty lines are skipped.

The `collection_id` on the resulting `EvalScenarios` is always auto-generated for JSONL — any `collection_id` field in the file is ignored. If you need a stable collection ID across runs, use JSON or YAML with the wrapped format.

### JSON

Two formats are accepted.

**Bare array** — simplest option:

```json
[
  {
    "id": "capital_france",
    "initial_query": "What is the capital of France?",
    "tasks": [
      {
        "task_type": "Assertion",
        "id": "mentions_paris",
        "context_path": "response",
        "operator": "Contains",
        "expected_value": "Paris"
      }
    ]
  }
]
```

**Wrapped with `collection_id`** — use this when you need a stable ID to correlate runs across time:

```json
{
  "collection_id": "my-qa-suite-v2",
  "scenarios": [
    {
      "id": "capital_france",
      "initial_query": "What is the capital of France?"
    }
  ]
}
```

JSON also accepts the output of `model_dump_json()` directly, so you can save a run and reload it for re-evaluation or diffing.

### YAML

Same two formats as JSON — bare array or wrapped with `collection_id`:

```yaml
collection_id: my-qa-suite-v2
scenarios:
  - id: capital_france
    initial_query: What is the capital of France?
    expected_outcome: Paris
    tasks:
      - task_type: Assertion
        id: mentions_paris
        context_path: response
        operator: Contains
        expected_value: Paris
  - id: water_formula
    initial_query: What is the chemical formula for water?
```

### Scenario fields

All `EvalScenario` fields are supported in files. `id` is auto-generated if omitted.

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | `string` | auto UUID | Stable ID for regression tracking |
| `initial_query` | `string` | required | First prompt sent to your agent |
| `predefined_turns` | `string[]` | `[]` | Follow-up queries for multi-turn scenarios |
| `expected_outcome` | `string` | — | Available as `${expected_outcome}` in task templates |
| `simulated_user_persona` | `string` | — | For simulated multi-turn agents |
| `termination_signal` | `string` | — | Signal to stop a simulated multi-turn run |
| `max_turns` | `int` | `10` | Max turns for simulated agents |
| `tasks` | `task[]` | `[]` | Scenario-level tasks (flat array with `task_type`) |
| `metadata` | `object` | — | Arbitrary key-value data, not used by the evaluator |

---

## Subclassing `EvalOrchestrator`

Use `agent_fn` for simple synchronous agents. Subclass when you need async execution, stateful history, or custom setup per scenario.

```python
class MyAgentEval(EvalOrchestrator):
    def execute_agent(self, scenario: EvalScenario) -> str:
        history = []
        response = my_stateful_agent.run(scenario.initial_query, history=history)
        history.append({"role": "user", "content": scenario.initial_query})
        history.append({"role": "assistant", "content": response})

        for turn in scenario.predefined_turns:
            response = my_stateful_agent.run(turn, history=history)
            history.append({"role": "user", "content": turn})
            history.append({"role": "assistant", "content": response})

        return response


results = MyAgentEval(queue=queue, scenarios=scenarios).run()
```

### Lifecycle hooks

Override these to add logging or post-processing without touching core execution:

```python
class MyEval(EvalOrchestrator):
    def on_scenario_start(self, scenario: EvalScenario) -> None:
        print(f"Starting: {scenario.id}")

    def on_scenario_complete(self, scenario: EvalScenario, response: str) -> None:
        print(f"Done: {scenario.id}")

    def on_evaluation_complete(self, results: ScenarioEvalResults) -> ScenarioEvalResults:
        results.save("latest_run.json")
        return results
```

Hook order per scenario: `on_scenario_start` → `execute_agent` → `on_scenario_complete`. `on_evaluation_complete` fires once after all scenarios finish.

---

## Saving, loading, and comparing results

```python
# Save a baseline
results = orch.run()
results.save("baseline_v1.json")

# Later — compare a new run against it
baseline = ScenarioEvalResults.load("baseline_v1.json")
new_results = orch.run()

comparison = new_results.compare_to(baseline, regression_threshold=0.05)
comparison.as_table()

if comparison.regressed:
    print(f"Regressed: {comparison.regressed_aliases}")
    raise SystemExit(1)
```

`regression_threshold` is the minimum pass-rate drop (0–1) that counts as a regression. Default is `0.05`.

`ScenarioComparisonResults` also serializes:

```python
comparison.save("comparison.json")
loaded = ScenarioComparisonResults.load("comparison.json")
```

### `ScenarioEvalResults` reference

| Property | Type | Description |
|---|---|---|
| `metrics.overall_pass_rate` | `float` | Mean pass rate across all datasets + scenario level (0–1) |
| `metrics.workflow_pass_rate` | `float` | Mean pass rate across sub-agent profile evaluations |
| `metrics.dataset_pass_rates` | `Dict[str, float]` | Per-alias pass rate, e.g. `{"retriever": 0.9}` |
| `metrics.scenario_pass_rate` | `float` | Fraction of scenarios where all tasks passed |
| `metrics.total_scenarios` | `int` | Total scenarios evaluated |
| `metrics.passed_scenarios` | `int` | Scenarios where every task passed |
| `dataset_results` | `Dict[str, EvalResults]` | Full per-alias evaluation results |
| `scenario_results` | `List[ScenarioResult]` | Per-scenario task results |

```python
results.as_table()
results.as_table(show_workflow=True)  # include the Workflow Summary table

detail = results.get_scenario_detail("rag_basics")
print(detail.pass_rate)
print(detail.passed)
```

For a full explanation of what each table shows, see [Reading your results](./reading-results.md).

---

## Working with pre-generated records

If you have records from a previous run or a separate data pipeline (no live agent needed), use `EvalDataset` instead. It takes `EvalRecord` objects directly alongside evaluation tasks.

→ [EvalDataset reference](./eval-dataset.md)
