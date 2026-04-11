## Agent Evaluation

Agent outputs are stochastic. The same prompt produces different responses across runs, model versions drift, and "it worked in testing" stops being a meaningful claim once your agent is in production. You need a systematic way to measure quality: before you ship and while you're running.

Scouter's eval framework gives you two views of your agent, captured in a single run.

---

## Two ways to look at an agent

Think of your agent like a car. A car either gets you from A to B or it doesn't. That's all a passenger cares about. But from the mechanic's perspective, the car arriving home doesn't mean it's healthy. It might have gotten there with a misfiring cylinder or low oil pressure. The passenger's view and the mechanic's view are both necessary, and neither replaces the other.

Scouter formalizes this split:

**Scenario evaluation (the passenger's view)** treats your agent as a black box. Given this input, did the agent produce the right output? You don't care how it got there (which tools it called, how it routed between sub-agents, or what retrieval quality looked like). You only care that the response was correct. This is your end-to-end quality signal.

**Workflow evaluation (the mechanic's view)** opens the hood. Each sub-agent in your pipeline can emit structured records during execution: intermediate outputs, tool results, context data. Workflow tasks evaluate those records, giving you per-component health signals independent of whether the final output passed.

A single `EvalOrchestrator` run produces both. You don't have to choose between them.

---

## Offline vs. online

**Offline evaluation** runs your agent against a fixed set of test scenarios before deployment. Use it to gate releases, catch regressions between model versions, and establish a quality baseline to compare future runs against.

**Online evaluation** samples production traffic and evaluates it asynchronously on the Scouter server. It catches distribution shift and real-world edge cases that don't show up in curated test scenarios.

The task definitions are the same in both modes. Write them once and reuse them. The difference is execution context: batch before deploy vs. sampled stream after deploy.

---

## Start here

| You have... | Go to |
|-------------|-------|
| A callable agent function | [Offline evaluation](./offline-evaluation.md) |
| Scenarios in a file (JSONL, JSON, YAML) | [Loading from a file](./offline-evaluation.md#loading-scenarios-from-a-file) |
| Pre-generated records (no live agent) | [EvalDataset](./eval-dataset.md) |
| A production service to monitor | [Online evaluation](./online-evaluation.md) |

For most users: start with [offline evaluation](./offline-evaluation.md).

---

## Task types

Tasks are what Scouter runs against your agent's outputs or records. They work the same whether you're evaluating offline or online.

| Task | What it checks | Cost |
|------|---------------|------|
| `AssertionTask` | Deterministic rules: format, threshold, presence, pattern matching | None |
| `LLMJudgeTask` | Semantic quality (relevance, faithfulness, tone) via an LLM call | One additional LLM call |
| `TraceAssertionTask` | Span properties: execution order, retry counts, token budgets | None |
| `AgentAssertionTask` | Tool calls and response structure: which tools ran, with what args, what they returned | None |

Tasks can depend on each other and act as conditional gates to prevent expensive downstream calls when preconditions fail. Full reference: [Evaluation tasks](./tasks.md) · [Conditional gates](./gates.md).
