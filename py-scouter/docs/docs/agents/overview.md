# Agent evaluation

Traditional ML models have labeled test sets and well-understood metrics — accuracy, F1, AUC. You compute a number, compare it to a threshold, and ship or don't. Agents are different. Their outputs are stochastic, shaped by tool availability, context windows, and orchestration logic that changes independently of the model itself. The same prompt can produce different tool call sequences across runs, and "it worked when I tested it" is the agent equivalent of "works on my machine."

You need two things: a way to measure quality before you ship, and a way to keep measuring after you ship. Those are different problems with different solutions, and Scouter's eval framework handles both.

---

## Scenario vs. workflow evaluation

Agents are pipelines, not functions. A correct final answer can mask broken intermediate steps — a retriever that returned irrelevant documents, a classifier that guessed right by accident, a sub-agent that burned 10x more tokens than it should have. If you only check the final output, you won't catch these until they compound into a visible failure.

Think of it like a car. A car either gets you from A to B or it doesn't — that's all a passenger cares about. But a mechanic cares about what happened under the hood. The car arriving home doesn't mean it's healthy. It might have gotten there with a misfiring cylinder or low oil pressure.

Scouter formalizes this split:

**Scenario evaluation (the passenger's view)** treats your agent as a black box. Given this input, did the agent produce the right output? You define tasks on your `EvalScenario` and they run against the agent's final response. This is your end-to-end quality signal.

**Workflow evaluation (the mechanic's view)** opens the hood. Each sub-agent in your pipeline emits structured `EvalRecord`s during execution — intermediate outputs, tool results, context data. Tasks on your `AgentEvalProfile` evaluate those records, giving you per-component health signals independent of whether the final output passed.

Both run in a single [`EvalOrchestrator`](./offline-evaluation.md) pass. You don't have to choose between them, and you'll want both. A passing scenario with failing workflow tasks means you got lucky, not that your agent is healthy.

The tasks and profiles you define for these two views are the same components whether you're running offline batch evaluation or online production monitoring. Define your evaluation once, reuse it everywhere. See [reading your results](./reading-results.md) for how the two views show up in the output tables.

---

## Offline and online evaluation

### Offline

Offline evaluation runs your agent against a fixed set of test scenarios before deployment. You define scenarios (input, expected outcome, tasks), point them at your agent function, and `EvalOrchestrator` handles the execution loop — running the agent, collecting records, evaluating tasks, and producing results.

Use it to gate releases, catch regressions between model versions or prompt changes, and establish a quality baseline you can compare future runs against. The comparison API diffs pass rates across runs and flags regressions above a configurable threshold.

If you already have records from a previous run or a production log export and don't need a live agent, [`EvalDataset`](./eval-dataset.md) is the lighter-weight option. Same task engine, no orchestrator.

### Online

Online evaluation samples production traffic and evaluates it asynchronously on the Scouter server. No impact on your application's latency. Records are inserted into a non-blocking queue and the server picks them up on its own schedule.

This is how you catch the things that don't show up in curated test scenarios: distribution shift, real-world edge cases, gradual quality degradation after a model provider update you didn't know about. `AgentEvalProfile` controls the sample ratio, and alert dispatch (Slack, OpsGenie, or Console) fires when evaluation results cross your configured thresholds.

### Same tasks, both modes

The task definitions are identical across offline and online. Write them once and use them in both contexts. An assertion that gates a release can monitor production without changes. This is deliberate because your offline quality bar and your production quality bar should be the same bar.

---

## What you get

- **Four task types.** Deterministic assertions, LLM-powered semantic judges, OpenTelemetry span assertions, and agent-specific tool call / response checks. All four work in both offline and online modes. → [Evaluation tasks](./tasks.md)

- **Bring your own context.** `EvalRecord` takes a freeform dict. Put whatever you want in it: model outputs, metadata, ground truth labels, intermediate results. Tasks read from it via `context_path` (dot-notation into nested fields). No fixed schema to conform to.

- **Dependency chains and conditional gates.** Tasks can depend on upstream results and act as gates that short-circuit expensive downstream work. If a format check fails, the LLM judge never runs. → [Conditional gates](./gates.md)

- **Multi-agent evaluation.** One profile per sub-agent in your pipeline. Each gets its own task set, its own results, and its own pass rate. → [Multi-agent setup](./offline-evaluation.md#multi-agent-setup)

- **Regression comparison.** Save results from a known-good run, then diff against new runs. The comparison flags regressions above a configurable threshold and tells you which aliases degraded. → [Comparing runs](./offline-evaluation.md#saving-loading-and-comparing-results)

- **Scheduled alerting.** Online profiles evaluate on a cron schedule and dispatch alerts when pass rates drop below your baseline. Slack, OpsGenie, and Console are supported out of the box. → [Online evaluation](./online-evaluation.md)

- **Portable definitions.** Profiles, tasks, and thresholds move between offline batch runs and online production monitoring without modification.

---

## Where to start

| What you have | Where to go |
|---|---|
| A callable agent function and test scenarios | [Offline evaluation](./offline-evaluation.md) |
| Scenarios in a file (JSONL, JSON, YAML) | [Loading from a file](./offline-evaluation.md#loading-scenarios-from-a-file) |
| Pre-generated records, no live agent | [EvalDataset](./eval-dataset.md) |
| A deployed agent you want to monitor | [Online evaluation](./online-evaluation.md) |

If you're unsure, start with [offline evaluation](./offline-evaluation.md). It's the fastest way to see how tasks, scenarios, and results fit together.

---

## Task types

Tasks are what Scouter runs against your agent's outputs or records. They work the same whether you're evaluating offline or online.

| Task | What it checks | Cost |
|------|---------------|------|
| [`AssertionTask`](./tasks.md#assertiontask) | Deterministic rules: format, threshold, presence, pattern matching | None |
| [`LLMJudgeTask`](./tasks.md#llmjudgetask) | Semantic quality (relevance, faithfulness, tone) via an LLM call | One LLM call |
| [`TraceAssertionTask`](./tasks.md#traceasserttiontask) | Span properties: execution order, retry counts, token budgets | None |
| [`AgentAssertionTask`](./tasks.md#agentassertiontask) | Tool calls and response structure: which tools ran, with what args, what they returned | None |

Tasks can depend on each other and act as conditional gates to prevent expensive downstream work when preconditions fail. Full reference: [Evaluation tasks](./tasks.md) · [Conditional gates](./gates.md).
