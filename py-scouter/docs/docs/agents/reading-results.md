# Reading your eval results

After `results.as_table()`, you'll see up to three tables. Here's what each one measures and why they're separate.

---

## The two evaluation loops

`EvalOrchestrator` runs two independent evaluation loops per scenario:

```
EvalOrchestrator.run()
│
├── For each scenario:
│   ├── agent runs
│   │   └── sub-agents emit EvalRecords          ← Workflow Summary evaluates these
│   └── agent returns a final response string     ← Scenario Results evaluates this
│
└── Aggregate Metrics = rolled-up numbers from both loops
```

**Scenario evaluation** is the passenger's view: did the car get from A to B? Tasks defined in `EvalScenario(tasks=[...])` run against the agent's final response string.

**Workflow evaluation** is the mechanic's view: are the internals healthy? Tasks defined in `AgentEvalProfile(tasks=[...])` run against the `EvalRecord` each sub-agent emitted during execution.

They measure different things. A scenario can pass (good final output) while workflow tasks fail — a sub-agent produced low-quality intermediate results but the final answer happened to be correct. You got lucky. Tracking both is the point.

---

## Aggregate metrics

Printed by `results.as_table()`. Rolled-up numbers from both loops.

| Metric | What it measures |
|--------|-----------------|
| **Overall Pass Rate** | Mean pass rate across both the scenario and workflow loops |
| **Workflow Pass Rate** | Mean pass rate across all sub-agent profile evaluations |
| **Scenario Pass Rate** | Fraction of scenarios where every scenario task passed |
| **Total Scenarios** | Number of scenarios run |
| **Passed Scenarios** | Scenarios where every scenario task passed |

If you haven't defined any `AgentEvalProfile` tasks, Workflow Pass Rate is omitted and Overall Pass Rate reflects only the scenario loop.

---

## Scenario results

One row per scenario task. This is the black-box view: output correctness only.

Tasks come from `EvalScenario(tasks=[...])`. They run against the string `execute_agent()` returned for that scenario.

```python
EvalScenario(
    id="capital_question",
    initial_query="What is the capital of France?",
    tasks=[
        AssertionTask(
            id="response_not_empty",
            context_path="response",
            operator=ComparisonOperator.IsString,
            expected_value=True,
        )
    ],
)
```

The `response` context key is populated automatically from the string your agent returns. No manual wiring needed.

---

## Workflow summary

Printed when you call `results.as_table(show_workflow=True)`. One row per `EvalRecord` emitted by a sub-agent across all scenarios.

Tasks come from `AgentEvalProfile(tasks=[...])`, the profile attached to your `ScouterQueue`. Each sub-agent that calls `span.add_queue_item(alias, record)` during execution produces rows here.

| Column | What it shows |
|--------|--------------|
| **Scenario ID** | Which scenario produced this record |
| **Record UID** | Last 8 chars of the record UUID (enough to distinguish records within a scenario) |
| **Alias** | The sub-agent name (matches what you passed to `span.add_queue_item`) |
| **Task** | Which profile task was evaluated |
| **Passed** | Whether that task passed for this record |
| **Pass Rate** | Pass rate across all tasks for this record |

```python
# During agent execution, a sub-agent emits a record:
span.add_queue_item(
    "retriever",
    EvalRecord(context={"results": {"count": 5, "source": "arxiv"}}),
)

# AgentEvalProfile defines what to check on that record:
AgentEvalProfile(
    alias="retriever",
    tasks=[
        AssertionTask(
            id="has_results",
            context_path="results.count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        )
    ],
)
```

With 3 scenarios and 2 sub-agents each emitting 1 record with 2 tasks, you get 12 Workflow Summary rows.

---

## Comparing runs

`ScenarioEvalResults` can be saved, loaded, and compared across runs:

```python
# Save a baseline after a known-good run
results.save("baseline_v1.json")

# Later — after a model update, prompt change, etc.
baseline = ScenarioEvalResults.load("baseline_v1.json")
new_results = orch.run()

comparison = new_results.compare_to(baseline, regression_threshold=0.05)
comparison.as_table()

if comparison.regressed:
    print(f"Regressed on: {comparison.regressed_aliases}")
    raise SystemExit(1)
```

`regression_threshold` is the minimum pass-rate drop that counts as a regression. Default is `0.05`, a 5-point drop. `ScenarioComparisonResults` also serializes to JSON.
