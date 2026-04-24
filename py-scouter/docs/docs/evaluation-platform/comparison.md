# Platform comparison

*Comparison current as of April 2026. Competitor features change frequently — verify against current docs before making decisions.*

This page compares Scouter's evaluation platform against five alternatives: LangSmith, Langfuse, MLflow, Datadog LLM Observability, and Google ADK. We focus on architectural differences and trade-offs rather than feature checklists. Every platform on this list does something well; the question is which set of trade-offs matches your requirements.

For the problem Scouter's evaluation platform solves and how it works, see the [evaluation platform overview](./index.md).

---

## The short version

Most platforms cover either offline evaluation (pre-deployment testing) or online evaluation (production monitoring) well. Few cover both, and fewer still let you share the same task definitions across both modes without re-wiring configuration in a different system.

Trace-based evaluation — asserting on span properties as part of the same pipeline that checks output quality — is treated as a separate observability concern by most platforms. You view traces in one tool and run evaluations in another.

Agent-specific assertions (tool call verification, response structure validation, vendor-agnostic format parsing) are either missing, LLM-judge-only, or vendor-locked in most alternatives.

The comparison table below covers the specifics. The sections that follow go deeper on each platform.

---

## Comparison table

| Capability | Scouter | LangSmith | Langfuse | MLflow | Datadog | Google ADK |
|---|---|---|---|---|---|---|
| **Offline evaluation** | Yes (SDK) | Yes (SDK) | Yes (SDK) | Yes (SDK) | No | Yes (CLI/pytest) |
| **Online evaluation** | Yes (SDK) | Yes (UI rules) | Yes (UI config) | Yes (Databricks) | Yes (primary) | No |
| **Unified task definitions** | Yes — same objects, both modes | Mostly — same callables, wired differently | Partially — same templates, different config | Yes — stated design goal | N/A (online only) | N/A (offline only) |
| **Deterministic assertions** | 46 comparison operators | Via custom evaluators | Via external pipelines | Open feature request | No | Yes (trajectory score) |
| **LLM-as-judge** | Yes, multi-vendor | Yes | Yes | Yes | Yes | No built-in |
| **Trace/span assertions** | Yes, in eval pipeline | Trajectory eval | Observation-level scoring | Trace-aware scorers | Inherent (all evals on traces) | Test-time only |
| **Agent tool-call assertions** | 14 deterministic assertion types | Trajectory matching | No built-in | LLM-based (ToolCallCorrectness) | LLM-based (Tool Selection) | Deterministic trajectory |
| **Vendor auto-detection** | OpenAI, Anthropic, Google | OpenAI + LangChain | No built-in | Via MLflow tracing | Via Datadog SDK | Google ADK only |
| **Dependency DAGs** | Yes, with conditional gates | No | No | No | No | No |
| **Conditional gates** | Yes — skip expensive tasks on cheap failures | No | No | No | No | No |
| **Config approach** | SDK + config files | SDK + UI | SDK + UI | SDK | UI only | Config files + pytest |
| **Deployment** | Self-hosted | SaaS + self-hosted (Enterprise) | SaaS + self-hosted (OSS) | OSS + Databricks | SaaS only | OSS (local/CI) |
| **Pricing model** | Self-hosted (infra only) | Per-seat + per-trace | Per-usage-unit; self-host free | OSS free; Databricks DBU | Per-day add-on (~$120/day) | Free (OSS) |

---

## Per-platform analysis

### LangSmith

LangSmith is LangChain's observability and evaluation platform. It has the broadest feature set of the SaaS options, with strong offline evaluation via SDK, online evaluation via UI-configured automation rules, and trajectory matching for agent tool calls through the separate `agentevals` package.

**What it does well.** Offline evaluation integrates naturally with pytest. The evaluator callable model is clean — write a Python function, use it in `evaluate()`, and the same function works in online automation rules. Trajectory matching (`create_trajectory_match_evaluator`) gives you deterministic tool call sequence verification in STRICT or UNORDERED modes. Multi-turn conversation evaluation is a first-class concept.

**Where it gets complicated.** Online evaluation is configured through UI automation rules, not the SDK. You write the evaluator in code but wire it to production traces through the web app. If your team manages infrastructure as code and wants evaluation config in version control alongside agent config, that split is friction. LangSmith's trajectory matching understands OpenAI message format and LangChain's BaseMessage — other vendor formats require normalization.

**Pricing.** $39/seat/month on the Plus plan, with per-trace overage ($2.50–$5.00 per 1K traces depending on retention). Self-hosted is Enterprise-only. At scale, per-trace costs can grow fast if you're sampling a large share of production traffic.

---

### Langfuse

Langfuse is an open-source LLM observability platform acquired by ClickHouse in early 2026. Self-hostable under MIT license, which makes it the most accessible option for teams that need to keep data on-premises.

**What it does well.** The self-hosted story is real — same codebase as cloud, runs on ClickHouse + Redis + S3. Scores attach at the trace or observation (span) level, so you can run different evaluators on different parts of a trace. The evaluator template library (via RAGAS partnership) covers common patterns: context relevance, hallucination detection, SQL semantic equivalence. Human annotation workflows are built in.

**Where it falls short for agent eval.** No built-in agent-specific assertions. Tool call verification, response structure validation, and trajectory matching all require custom external pipelines — fetch traces via API, evaluate externally, push scores back. Online evaluators are configured in the UI; offline experiments use the SDK. The evaluator *templates* are reusable across modes, but the wiring is different. No dependency chains between evaluators — each runs independently.

**Pricing.** Cloud tiers from free (50K units/month) to Enterprise ($2,499/month). Self-hosted is free; you pay infrastructure costs (roughly $3–4K/month at medium scale for ClickHouse + Redis + S3).

---

### MLflow

MLflow's GenAI evaluation is the closest to Scouter's design philosophy: scorers are Python objects that work in both offline `evaluate()` calls and production monitoring. The "same scorer in dev and prod" principle is a stated design goal.

**What it does well.** Trace-aware evaluation is deep — scorers receive full MLflow Trace objects with access to every span, tool call, and intermediate message. `ToolCallCorrectness` compares actual tool calls against expected calls with fuzzy (LLM-powered) semantic matching. `ToolCallEfficiency` scores whether the agent used tools without waste. The scorer model is clean and extensible.

**Where it doesn't go far enough.** Tool call assertions are LLM-based, not deterministic. There's an open feature request (#20827) for "Tier 1 deterministic scorers" — rule-based structural checks that run before LLM judges — but it's not implemented yet. No dependency chains between scorers. No conditional gates to skip expensive evaluations when cheap preconditions fail. Production monitoring requires Databricks; the open-source MLflow server doesn't run scheduled scorer evaluations on its own.

**Pricing.** MLflow is Apache 2.0, free to self-host. Production monitoring is a Databricks platform feature with DBU-based pricing.

---

### Datadog LLM Observability

Datadog approaches agent evaluation from the observability side. All evaluation runs on production traces — there is no offline evaluation workflow.

**What it does well.** If your agents are already instrumented with Datadog's SDK, evaluation is turnkey. Managed evaluation templates cover common patterns: topic relevancy, sentiment, failure to answer, toxicity, prompt injection. Agent-specific templates (Tool Selection, Tool Argument Correctness, Goal Completeness) evaluate tool use quality. Everything lives in the same UI as your other Datadog monitoring, which means existing alerting, dashboards, and incident workflows apply.

**What it doesn't do.** No offline evaluation. You can't run evaluators against a test dataset before deploying. No deterministic assertions — all evaluations are LLM-as-judge, including tool call validation. No SDK-driven evaluation configuration; everything is UI-configured. If you want evaluation config in version control and CI, this is the wrong tool.

**Pricing.** LLM Observability is a paid add-on at roughly $120/day when activated. Teams report 40–200% bill increases when adding LLM monitoring to an existing Datadog account. Custom LLM-as-judge evaluations incur additional LLM API costs in your own provider account. New pricing takes effect May 2026.

---

### Google ADK

ADK is a development framework, not a platform. Its evaluation capabilities are focused on pre-deployment testing — running agents against test cases in CI/CD or local development.

**What it does well.** `tool_trajectory_avg_score` is a clean, deterministic tool call sequence evaluator with three matching modes: EXACT (identical sequence), IN_ORDER (subsequence in order), and ANY_ORDER (set membership). Test configuration is file-driven (JSON), which integrates well with version control. Pytest integration means evaluation runs in CI without additional infrastructure.

**What it doesn't cover.** No online evaluation. No production monitoring. No LLM-as-judge built in. No trace storage or observability. For production evaluation, Google recommends integrating with third-party platforms (Arize, LangWatch, Langfuse). ADK gives you the development-loop piece; you bring everything else.

**Pricing.** Free (Apache 2.0). You pay for LLM API calls during test runs.

---

## Where Scouter fits

Scouter's position is opinionated: evaluation is a single system, not two systems glued together. Offline and online evaluation use the same task objects, the same comparison operators, the same dependency DAGs. A task that gates a release in CI monitors the same quality dimension in production without changes.

The specific gaps Scouter fills relative to alternatives:

**Deterministic assertions at scale.** 46 comparison operators covering numeric, string, collection, type, format, and range validations. Most alternatives either lack deterministic assertions entirely (Datadog, MLflow's current state) or provide them only for narrow use cases (LangSmith trajectory matching, ADK trajectory scoring). When you need to check "response is valid JSON, contains fields X and Y, confidence is above 0.85, and the summary is under 200 characters" — that's four `AssertionTask` instances, no LLM calls, sub-millisecond execution.

**Dependency DAGs with conditional gates.** No other platform in this comparison supports task dependencies or conditional gates. If a format check fails, an LLM judge shouldn't run — it's wasted tokens and latency. Scouter's `condition=True` on any task makes it a gate; failure skips all downstream dependents. The engine topologically sorts the DAG and runs independent tasks in parallel.

**Trace assertions in the evaluation pipeline.** `TraceAssertionTask` operates on OpenTelemetry span data — execution order, retry counts, token budgets across spans, latency SLAs, error counts — as part of the same task DAG that checks output quality. Other platforms either treat trace evaluation as separate from output evaluation (Langfuse, Datadog) or don't support span-level assertions at all (ADK).

**Vendor-agnostic agent assertions.** `AgentAssertionTask` with 14 assertion variants auto-detects OpenAI, Anthropic, and Google response formats. Tool call verification (called, not called, called with args, call sequence, call count), argument extraction, response content, model, finish reason, token counts — all deterministic, no LLM call required. LangSmith's trajectory matching supports OpenAI and LangChain formats. MLflow's ToolCallCorrectness requires an LLM call for fuzzy matching. Datadog's tool evaluations are LLM-based.

**SDK-driven configuration for both modes.** Profiles, tasks, and thresholds are defined in Python (or JSON/YAML config files) and registered via the SDK. No UI-driven configuration that lives outside version control. LangSmith's online evaluation requires UI automation rules. Langfuse's online evaluators are UI-configured. Datadog is entirely UI-driven. Scouter's approach means evaluation config lives next to agent config, goes through code review, and deploys through the same CI/CD pipeline.

**Self-hosted with no per-trace pricing.** The server is a Rust binary backed by PostgreSQL and Delta Lake. You pay for infrastructure, not per-span or per-trace fees. At high trace volumes, this matters — a SaaS platform charging $2.50–$5.00 per 1K traces or $120/day in add-on fees changes the cost calculus for comprehensive production monitoring.

---

## What Scouter doesn't do

Being honest about gaps:

- **No managed SaaS.** You deploy and operate the server yourself. If your team doesn't want to manage infrastructure, LangSmith or Langfuse Cloud are easier starting points.
- **No human annotation workflows.** LangSmith and Langfuse have built-in annotation UIs for human-in-the-loop evaluation. Scouter doesn't.
- **No built-in evaluator template library.** Langfuse's RAGAS integration and Datadog's managed templates give you pre-built evaluators for common patterns (hallucination detection, context relevance, toxicity). In Scouter, you define all evaluation logic yourself.
- **No conversation simulation.** MLflow's conversation simulation generates synthetic multi-turn conversations for testing. Scouter's offline evaluation runs against scenarios you provide — it doesn't generate them.
- **No pairwise comparison.** LangSmith supports pairwise comparison evaluators (comparing two outputs side-by-side). Scouter evaluates individual outputs against criteria.

Scouter focuses on the evaluation engine — the part that decides whether an agent's behavior meets your quality bar. Test scenarios, annotation workflows, and synthetic data generation are your responsibility.
