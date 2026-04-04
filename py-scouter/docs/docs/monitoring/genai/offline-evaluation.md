# Offline Evaluation

Offline evaluation runs your agent against a fixed set of test scenarios and measures quality across every sub-agent — before anything reaches production. Use it to catch regressions between model versions, validate prompt changes, and build a quality baseline to compare future runs against.

---

## Two Approaches

| | `EvalOrchestrator` | `EvalDataset` |
|---|---|---|
| **Use when** | You have a callable agent to invoke | You have pre-generated records |
| **Input** | `EvalScenarios` + your agent function | `EvalRecord` list + evaluation tasks |
| **Output** | `ScenarioEvalResults` (save/load/compare) | `EvalResults` |
| **Multi-agent** | Yes — one `GenAIEvalProfile` per sub-agent | Flat — single task list |
| **Regression testing** | Built-in `compare_to()` | Not supported |

For most agent workflows, **start with `EvalOrchestrator`**. `EvalDataset` is covered at the bottom for cases where you already have records.

---

## Quick Start

```python
from scouter.drift import GenAIEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
)
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import TestSpanExporter, init_tracer

# 1. Define what to evaluate about your agent's outputs
profile = GenAIEvalProfile(
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
queue = ScouterQueue.from_profile(profile=[profile],wait_for_startup=True)

# 3. Initialize a tracer — your agent emits EvalRecords inside traced spans
tracer = init_tracer( service_name="my-eval", scouter_queue=queue)

# 4. Your agent function — emits an EvalRecord, returns a response string
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
orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=my_agent)
results = orch.run()
results.as_table()
```

---

## How It Works

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

`EvalRunner` is the Rust engine. `EvalOrchestrator` is the Python lifecycle wrapper around it.

### 3-level evaluation

`EvalRunner.evaluate()` runs three levels in sequence:

**Level 1 — Sub-agent evaluation (holistic)**

For each alias (sub-agent), all records collected across *all scenarios* are flattened into a single `EvalDataset` and evaluated together. This gives you a holistic quality signal per sub-agent — independent of which scenario produced which record.

```
alias "retriever" → 5 records (one per scenario) → EvalDataset → EvalResults
alias "synthesizer" → 5 records → EvalDataset → EvalResults
```

**Level 2 — Scenario-level evaluation**

For each scenario that has `tasks`, a single `EvalRecord` is built from the scenario context (agent response + `expected_outcome`) and evaluated against those tasks. `TraceAssertionTask`s are resolved by matching `trace_id`s from the scenario's records to spans in the capture buffer.

```
scenario "capital_question" → build record from {response, expected_outcome}
  → evaluate scenario tasks → ScenarioResult { passed, pass_rate }
```

**Level 3 — Aggregate metrics**

```
EvalMetrics:
  overall_pass_rate     # mean across all dataset + scenario pass rates
  dataset_pass_rates    # per-alias pass rate (e.g. {"retriever": 0.9, "synthesizer": 0.6})
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

## Core Concepts

### `EvalScenario`

A single test case. At minimum, supply `initial_query`.

```python
EvalScenario(
    id="scenario_id",                        # stable ID for regression tracking
    initial_query="Summarize this article.",
    expected_outcome="A 2-sentence summary.", # available as ${expected_outcome} in tasks
    tasks=[                                  # evaluated against the agent's final response
        AssertionTask(
            id="response_not_empty",
            context_path="response",
            operator=ComparisonOperator.IsString,
            expected_value=True,
        ),
    ],
)
```

Scenario `tasks` evaluate the agent's **final response string**. They are separate from sub-agent profile tasks, which evaluate each sub-agent's `EvalRecord` context.

### `GenAIEvalProfile`

Defines evaluation tasks for one sub-agent. The `alias` must match what you pass to `span.add_queue_item(alias, ...)`.

```python
GenAIEvalProfile(
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

## Multi-Agent Setup

One `GenAIEvalProfile` per sub-agent. Register all profiles on the queue.

```python
from scouter.drift import GenAIEvalProfile
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
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor, TestSpanExporter, init_tracer

retriever_profile = GenAIEvalProfile(
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

synthesizer_profile = GenAIEvalProfile(
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
    wait_for_startup=True,
)

# ScouterInstrumentor is needed when using TraceAssertionTask
instrumentor = ScouterInstrumentor().instrument(scouter_queue=queue)

tracer = init_tracer(
    service_name="my-agent",
    scouter_queue=queue,
)


def retriever_callback(query: str) -> dict:
    """Sub-agent: retrieve documents and emit a record."""
    with tracer.start_as_current_span("retriever_call") as span:
        results = {"count": 5, "source": "internal_db"}
        span.add_queue_item("retriever", EvalRecord(context={"results": results}))
    return results


def synthesizer_callback(query: str, context: dict) -> dict:
    """Sub-agent: synthesize a response and emit a record."""
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

orch = EvalOrchestrator(queue=queue, scenarios=scenarios, agent_fn=agent_fn)
results = orch.run()
results.as_table()

instrumentor.uninstrument()
```

`ScouterInstrumentor` is required when your profiles include `TraceAssertionTask`. If you only have `AssertionTask` and `LLMJudgeTask`, `init_tracer` alone is sufficient.

---

## Multi-Turn Scenarios

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

`agent_fn` receives each query in isolation — no conversation history is managed automatically. To handle stateful agents, subclass `EvalOrchestrator` and override `execute_agent`.

---

## Subclassing `EvalOrchestrator`

Use `agent_fn` for simple agents. Subclass when you need stateful execution or custom data emission per scenario.

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


orch = MyAgentEval(queue=queue, scenarios=scenarios)
results = orch.run()
```

### Lifecycle hooks

Override these to add logging or post-processing without changing core execution:

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

Hook order per scenario: `on_scenario_start` → `execute_agent` → `on_scenario_complete`. `on_evaluation_complete` fires once after all scenarios.

---

## Saving, Loading, and Comparing Results

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
| `metrics.dataset_pass_rates` | `Dict[str, float]` | Per-alias pass rate, e.g. `{"retriever": 0.9}` |
| `metrics.scenario_pass_rate` | `float` | Fraction of scenarios where all tasks passed |
| `metrics.total_scenarios` | `int` | Total scenarios evaluated |
| `metrics.passed_scenarios` | `int` | Scenarios where every task passed |
| `dataset_results` | `Dict[str, EvalResults]` | Full per-alias evaluation results |
| `scenario_results` | `List[ScenarioResult]` | Per-scenario task results |

```python
results.as_table()

detail = results.get_scenario_detail("rag_basics")
print(detail.pass_rate)
print(detail.passed)
```

---

## Using `EvalDataset` (Record-Based)

`EvalDataset` is for cases where you already have records — no agent callable required. You supply `EvalRecord` objects directly alongside evaluation tasks. It supports the same task types but does not produce `ScenarioEvalResults` or comparison output.

### Example: Appliance Customer Service Evaluation

This example shows conditional routing across multiple product categories using `condition=True` on `AssertionTask`.

#### Step 1: Generate Records

```python
import random
from typing import List, Literal

from pydantic import BaseModel
from scouter.evaluate import AssertionTask, ComparisonOperator, EvalDataset, LLMJudgeTask
from scouter.genai import Agent, Prompt, Provider
from scouter.queue import EvalRecord

categories = ["bath", "kitchen", "outdoor"]
ApplianceCategory = Literal["kitchen", "bath", "outdoor"]


class UserQuestion(BaseModel):
    question: str
    category: ApplianceCategory


class AgentResponse(BaseModel):
    answer: str
    product_recommendations: List[str]
    safety_notes: List[str]


def simulate_agent_interaction(num_questions: int) -> List[EvalRecord]:
    agent = Agent(Provider.Gemini)

    question_prompt = Prompt(
        messages=(
            "Generate a realistic customer question about one of three appliance "
            "categories: kitchen, bath, or outdoor. Category: ${category}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=UserQuestion,
    )
    response_prompt = Prompt(
        messages=(
            "You are a home appliance expert. Answer this customer question.\n\n"
            "Question: ${user_question}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=AgentResponse,
    )

    records = []
    for _ in range(num_questions):
        category = categories[random.randint(0, 2)]
        question = agent.execute_prompt(
            prompt=question_prompt.bind(category=category),
            output_type=UserQuestion,
        ).structured_output

        response = agent.execute_prompt(
            prompt=response_prompt.bind(user_question=question.question),
            output_type=AgentResponse,
        ).structured_output

        records.append(EvalRecord(context={
            "user_input": question.question,
            "agent_response": response.model_dump_json(),
        }))

    return records
```

#### Step 2: Define Evaluation Tasks

```python
from pydantic import BaseModel


class CategoryValidation(BaseModel):
    category: ApplianceCategory
    reason: str
    confidence: float


class KitchenExpertValidation(BaseModel):
    is_suitable: bool
    reason: str
    addresses_safety: bool
    technical_accuracy_score: int


# Base classification task — runs for every record
classification_task = LLMJudgeTask(
    id="category_classification",
    prompt=Prompt(
        messages=(
            "Classify the appliance category (kitchen, bath, outdoor).\n\n"
            "Question: ${user_input}\nResponse: ${agent_response}"
        ),
        model="gemini-2.5-flash-lite",
        provider="gemini",
        output_type=CategoryValidation,
    ),
    expected_value=None,
    operator=ComparisonOperator.IsNotEmpty,
    context_path="category",
)

# Kitchen validation chain — only runs when category_classification.category == "kitchen"
kitchen_tasks = [
    AssertionTask(
        id="is_kitchen_category",
        context_path="category_classification.category",
        operator=ComparisonOperator.Equals,
        expected_value="kitchen",
        depends_on=["category_classification"],
        condition=True,  # gates all downstream kitchen tasks
    ),
    LLMJudgeTask(
        id="kitchen_expert_validation",
        prompt=Prompt(
            messages=(
                "You are a kitchen appliance specialist. Evaluate this response.\n\n"
                "Question: ${user_input}\nResponse: ${agent_response}"
            ),
            model="gemini-2.5-flash-lite",
            provider="gemini",
            output_type=KitchenExpertValidation,
        ),
        expected_value=True,
        operator=ComparisonOperator.Equals,
        context_path="is_suitable",
        depends_on=["is_kitchen_category"],
    ),
    AssertionTask(
        id="kitchen_technical_score",
        context_path="kitchen_expert_validation.technical_accuracy_score",
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=7,
        depends_on=["kitchen_expert_validation"],
    ),
]
# Define bath_tasks and outdoor_tasks following the same pattern
```

#### Step 3: Assemble and Run

```python
records = simulate_agent_interaction(num_questions=10)

dataset = EvalDataset(
    records=records,
    tasks=[classification_task] + kitchen_tasks,  # + bath_tasks + outdoor_tasks
)

dataset.print_execution_plan()
results = dataset.evaluate()
results.as_table()
results.as_table(show_tasks=True)
```

### Conditional routing

Tasks with `condition=True` act as gates. When a conditional task fails, all downstream tasks that depend on it are skipped — no LLM calls are wasted on the wrong category.

```
category_classification (always runs)
    ├── is_kitchen_category (condition=True) → gates kitchen chain
    │     └── kitchen_expert_validation → kitchen_technical_score
    ├── is_bath_category (condition=True) → gates bath chain
    │     └── bath_expert_validation → bath_installation_score
    └── is_outdoor_category (condition=True) → gates outdoor chain
          └── outdoor_expert_validation → outdoor_durability_score
```

### Context flow

Each task only sees its `EvalRecord` base context plus the outputs of tasks it declares in `depends_on`. A task that does not declare a dependency cannot access that upstream task's output.

```python
# This task can read category_classification.category
AssertionTask(
    id="is_kitchen_category",
    context_path="category_classification.category",
    depends_on=["category_classification"],  # makes the output available
    ...
)

# This task can read kitchen_expert_validation.technical_accuracy_score
# but NOT category_classification (not in depends_on)
AssertionTask(
    id="kitchen_technical_score",
    context_path="kitchen_expert_validation.technical_accuracy_score",
    depends_on=["kitchen_expert_validation"],
    ...
)
```
